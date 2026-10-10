// SPDX-License-Identifier: Apache-2.0
//! In-memory, bounded RFC8259 JSON decoder/array encoder for P1.
//! The parser consumes a bounded frame by reference; no network or filesystem access.
use std::collections::BTreeMap;
use tram_model::{Datum, RecordEnvelope};
use crate::budget::{JSON_MAX_DEPTH, JSON_MAX_TOKENS, RAW_FILE_MAX, RECORDS_PER_SOURCE_MAX,
    ENCODED_RECORD_MAX, DECODED_SOURCE_MAX, SCRATCH_RUN_MAX};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JsonError { Malformed, InvalidUtf8, Schema, ResourceExhausted, UnsupportedType, PrecisionLoss }
/// The input frame is bounded by RAW_FILE_MAX *before* parser entry.
/// No separate unmetered whole-frame UTF-8 String or generic JSON Value is built.
pub fn decode(input: &[u8]) -> Result<Vec<BTreeMap<String,Datum>>, JsonError> {
    if input.len()>RAW_FILE_MAX {return Err(JsonError::ResourceExhausted);}
    std::str::from_utf8(input).map_err(|_|JsonError::InvalidUtf8)?;
    let mut parser=Parser { src:input, pos:0, depth:0, tokens:0, charged:0 };
    let rows=parser.document()?;
    parser.ws();
    if parser.pos!=input.len() {return Err(JsonError::Malformed);}
    Ok(rows)
}
struct Parser<'a> {
    src:&'a [u8],pos:usize,depth:usize,tokens:usize,charged:usize,
}
impl Parser<'_> {
    fn ws(&mut self){while matches!(self.src.get(self.pos),Some(b' '|b'\r'|b'\n'|b'\t')){self.pos+=1;}}
    fn consume(&mut self,b:u8)->bool {self.ws();if self.src.get(self.pos)==Some(&b){self.pos+=1;true}else{false}}
    fn expect(&mut self,b:u8)->Result<(),JsonError>{if self.consume(b){Ok(())}else{Err(JsonError::Malformed)}}
    fn budget(&mut self, bytes:usize)->Result<(),JsonError>{
        self.charged=self.charged.checked_add(bytes).ok_or(JsonError::ResourceExhausted)?;
        if self.charged>DECODED_SOURCE_MAX{return Err(JsonError::ResourceExhausted);}
        Ok(())
    }
    fn token(&mut self)->Result<(),JsonError>{
        self.tokens=self.tokens.checked_add(1).ok_or(JsonError::ResourceExhausted)?;
        if self.tokens>JSON_MAX_TOKENS{return Err(JsonError::ResourceExhausted);}
        Ok(())
    }
    fn enter(&mut self)->Result<(),JsonError>{
        self.depth+=1;
        if self.depth>JSON_MAX_DEPTH{return Err(JsonError::ResourceExhausted);}
        Ok(())
    }
    fn document(&mut self)->Result<Vec<BTreeMap<String,Datum>>,JsonError>{
        self.ws();
        let mut rows=Vec::new();
        if self.consume(b'['){
            self.enter()?;
            if !self.consume(b']'){
                loop {
                    if rows.len()>=RECORDS_PER_SOURCE_MAX{return Err(JsonError::ResourceExhausted);}
                    rows.push(self.object()?);
                    if self.consume(b']'){break;}
                    self.expect(b',')?;
                }
            }
            self.depth-=1;
        } else {
            rows.push(self.object()?);
        }
        Ok(rows)
    }
    fn object(&mut self)->Result<BTreeMap<String,Datum>,JsonError>{
        self.token()?;self.expect(b'{')?;self.enter()?;
        let mut object=BTreeMap::new();
        if !self.consume(b'}'){
            loop {
                let name=self.string()?;
                self.budget(name.len().saturating_add(120))?;
                self.expect(b':')?;
                let value=self.value()?;
                if object.insert(name,value).is_some(){return Err(JsonError::Malformed);}
                if self.consume(b'}'){break;}
                self.expect(b',')?;
            }
        }
        self.depth-=1;Ok(object)
    }
    fn value(&mut self)->Result<Datum,JsonError>{
        self.ws();self.token()?;
        self.budget(72)?;
        match self.src.get(self.pos).copied() {
            Some(b'"')=>Ok(Datum::String(self.string_inner()?)),
            Some(b'{')=>self.object().map(Datum::Object),
            Some(b'[')=>{
                self.expect(b'[')?;self.enter()?;
                let mut values=Vec::new();
                if !self.consume(b']'){
                    loop {
                        self.budget(24)?;
                        values.push(self.value()?);
                        if self.consume(b']'){break;}
                        self.expect(b',')?;
                    }
                }
                self.depth-=1;Ok(Datum::Array(values))
            }
            Some(b't')=>{self.keyword(b"true")?;Ok(Datum::Boolean(true))}
            Some(b'f')=>{self.keyword(b"false")?;Ok(Datum::Boolean(false))}
            Some(b'n')=>{self.keyword(b"null")?;Ok(Datum::Null)}
            Some(b'-'|b'0'..=b'9')=>self.number(),
            _=>Err(JsonError::Malformed),
        }
    }
    fn keyword(&mut self,word:&[u8])->Result<(),JsonError>{
        if self.src.get(self.pos..self.pos+word.len())==Some(word){self.pos+=word.len();Ok(())}
        else{Err(JsonError::Malformed)}
    }
    fn number(&mut self)->Result<Datum,JsonError>{
        let begin=self.pos;
        if self.src.get(self.pos)==Some(&b'-'){self.pos+=1;}
        match self.src.get(self.pos) {
            Some(b'0')=>{self.pos+=1;if matches!(self.src.get(self.pos),Some(b'0'..=b'9')){return Err(JsonError::Malformed);}},
            Some(b'1'..=b'9')=>{self.pos+=1;while matches!(self.src.get(self.pos),Some(b'0'..=b'9')){self.pos+=1;}},
            _=>return Err(JsonError::Malformed),
        }
        if matches!(self.src.get(self.pos),Some(b'.'|b'e'|b'E')) {
            // The exact P1 float mapping is not frozen. Do not round/coerce.
            return Err(JsonError::PrecisionLoss);
        }
        let text=std::str::from_utf8(&self.src[begin..self.pos]).map_err(|_|JsonError::InvalidUtf8)?;
        if let Ok(n)=text.parse::<i64>(){return Ok(Datum::Signed(n));}
        if let Ok(n)=text.parse::<u64>(){return Ok(Datum::Unsigned(n));}
        Datum::bigint(text).map_err(|_|JsonError::Malformed)
    }
    fn string(&mut self)->Result<String,JsonError>{self.token()?;self.string_inner()}
    fn string_inner(&mut self)->Result<String,JsonError>{
        self.expect(b'"')?;
        let mut out=String::new();
        let mut plain=self.pos;
        loop{
            let Some(&byte)=self.src.get(self.pos) else{return Err(JsonError::Malformed)};
            if byte==b'"'{
                if self.pos>plain{
                    let str_=std::str::from_utf8(&self.src[plain..self.pos]).map_err(|_|JsonError::InvalidUtf8)?;
                    self.budget(str_.len())?;out.push_str(str_);
                }
                self.pos+=1;return Ok(out);
            }
            if byte==b'\\'{
                if self.pos>plain{
                    let str_=std::str::from_utf8(&self.src[plain..self.pos]).map_err(|_|JsonError::InvalidUtf8)?;
                    self.budget(str_.len())?;out.push_str(str_);
                }
                self.pos+=1;
                let esc=*self.src.get(self.pos).ok_or(JsonError::Malformed)?;
                self.pos+=1;
                let ch=match esc {
                    b'"'=>'"',b'\\'=>'\\',b'/'=>'/',b'b'=>'\u{0008}',
                    b'f'=>'\u{000c}',b'n'=>'\n',b'r'=>'\r',b't'=>'\t',
                    b'u'=>{
                        let first=self.hex_u16()?;
                        if (0xd800..=0xdbff).contains(&first){
                            if self.src.get(self.pos..self.pos+2)!=Some(&b"\\u"[..]){
                                return Err(JsonError::Malformed);
                            }
                            self.pos+=2;
                            let last=self.hex_u16()?;
                            if !(0xdc00..=0xdfff).contains(&last){return Err(JsonError::Malformed);}
                            let scalar=0x10000+((u32::from(first)-0xd800)<<10)+(u32::from(last)-0xdc00);
                            char::from_u32(scalar).ok_or(JsonError::Malformed)?
                        }else{
                            if (0xdc00..=0xdfff).contains(&first){return Err(JsonError::Malformed);}
                            char::from_u32(u32::from(first)).ok_or(JsonError::Malformed)?
                        }
                    }
                    _=>return Err(JsonError::Malformed),
                };
                self.budget(ch.len_utf8())?;out.push(ch);plain=self.pos;
            } else {
                if byte<0x20{return Err(JsonError::Malformed);}
                self.pos+=1;
            }
        }
    }
    fn hex_u16(&mut self)->Result<u16,JsonError>{
        let mut n=0u16;
        for _ in 0..4 {
            let c=*self.src.get(self.pos).ok_or(JsonError::Malformed)?;
            let x=match c{
                b'0'..=b'9'=>c-b'0',
                b'a'..=b'f'=>c-b'a'+10,
                b'A'..=b'F'=>c-b'A'+10,
                _=>return Err(JsonError::Malformed),
            };
            n=(n<<4)|u16::from(x);self.pos+=1;
        }
        Ok(n)
    }
}
fn escaped(out:&mut Vec<u8>,text:&str,cap:usize)->Result<(),JsonError>{
    push(out,b"\"",cap)?;
    for c in text.chars(){
        match c {
            '"' => push(out,b"\\\"",cap)?,
            '\\'=>push(out,b"\\\\",cap)?,
            '\n'=>push(out,b"\\n",cap)?,
            '\r'=>push(out,b"\\r",cap)?,
            '\t'=>push(out,b"\\t",cap)?,
            c if (c as u32)<0x20 || (c as u32)>0x7f=>{
                if (c as u32)<=0xffff{
                    let hex=format!("\\u{:04x}",c as u32);
                    push(out,hex.as_bytes(),cap)?;
                }else{
                    let n=(c as u32)-0x10000;
                    let first=0xd800+(n>>10);
                    let second=0xdc00+(n&0x3ff);
                    let hex=format!("\\u{first:04x}\\u{second:04x}");
                    push(out,hex.as_bytes(),cap)?;
                }
            },
            _=>{let mut bytes=[0;4];push(out,c.encode_utf8(&mut bytes).as_bytes(),cap)?;}
        }
    }
    push(out,b"\"",cap)
}
fn push(out:&mut Vec<u8>,slice:&[u8],cap:usize)->Result<(),JsonError>{
    let n=out.len().checked_add(slice.len()).ok_or(JsonError::ResourceExhausted)?;
    if n>cap {return Err(JsonError::ResourceExhausted);}
    out.extend_from_slice(slice);Ok(())
}
fn encode_map(out:&mut Vec<u8>,map:&BTreeMap<String,Datum>,cap:usize,depth:usize)->Result<(),JsonError>{
    if depth>JSON_MAX_DEPTH{return Err(JsonError::ResourceExhausted);}
    push(out,b"{",cap)?;
    for (i,(key,value)) in map.iter().enumerate(){
        if i>0 {push(out,b",",cap)?;}
        escaped(out,key,cap)?;push(out,b":",cap)?;
        encode_datum(out,value,cap,depth+1)?;
    }
    push(out,b"}",cap)
}
fn encode_datum(out:&mut Vec<u8>,value:&Datum,cap:usize,depth:usize)->Result<(),JsonError>{
    if depth>JSON_MAX_DEPTH{return Err(JsonError::ResourceExhausted);}
    match value{
        Datum::Null=>push(out,b"null",cap),
        Datum::Boolean(v)=>push(out,if *v{b"true"}else{b"false"},cap),
        Datum::Signed(v)=>push(out,v.to_string().as_bytes(),cap),
        Datum::Unsigned(v)=>push(out,v.to_string().as_bytes(),cap),
        Datum::BigInteger(v)=>push(out,v.as_str().as_bytes(),cap),
        Datum::String(v)=>escaped(out,v,cap),
        Datum::Array(items)=>{
            push(out,b"[",cap)?;
            for (i,item) in items.iter().enumerate(){
                if i>0 {push(out,b",",cap)?;}
                encode_datum(out,item,cap,depth+1)?;
            }
            push(out,b"]",cap)
        }
        Datum::Object(map)=>encode_map(out,map,cap,depth+1),
        Datum::Bytes(_) | Datum::Timestamp(_) | Datum::Decimal(_) | Datum::Float(_)=>
            Err(JsonError::UnsupportedType),
    }
}
/// Enforce the 1MiB per-record output cap *before* creating an external sink.
pub fn encode_one(row:&BTreeMap<String,Datum>)->Result<Vec<u8>,JsonError>{
    let mut buf=Vec::new();encode_map(&mut buf,row,ENCODED_RECORD_MAX,0)?;
    Ok(buf)
}
/// Encoding emits one JSON array. No fsync/delivery or file I/O is performed.
pub fn encode_array(rows:&[RecordEnvelope])->Result<Vec<u8>,JsonError>{
    if rows.len()>RECORDS_PER_SOURCE_MAX{return Err(JsonError::ResourceExhausted);}
    let mut buf=Vec::new();push(&mut buf,b"[",SCRATCH_RUN_MAX)?;
    for (i,row) in rows.iter().enumerate(){
        if i>0{push(&mut buf,b",",SCRATCH_RUN_MAX)?;}
        let frame=encode_one(&row.data)?;
        push(&mut buf,&frame,SCRATCH_RUN_MAX)?;
    }
    push(&mut buf,b"]",SCRATCH_RUN_MAX)?;
    Ok(buf)
}
#[cfg(test)]
mod tests {
    use super::*;
    use tram_model::{Provenance,RunId,RecordId,SourceUnitId};
    fn row(map:BTreeMap<String,Datum>)->RecordEnvelope {
        let mut rec=RecordEnvelope::new(Provenance{
            run_id:RunId::new("test").unwrap(),
            source_unit_id:SourceUnitId::new("unit").unwrap(),
            record_id:RecordId::new("record").unwrap(),
            source_plugin:"local".into(),ingested_at:None,
        });rec.data=map;rec
    }
    #[test]
    fn comp_04_object_array_empty_and_scalar_rejection(){
        assert_eq!(decode(br#"{"a":1}"#).unwrap().len(),1);
        assert_eq!(decode(br#"[{"a":1},{"a":2}]"#).unwrap().len(),2);
        assert_eq!(decode(b"[]").unwrap().len(),0);
        for bytes in [b"42".as_slice(),b"null",b"[42]",b"[null]",b"[{} ,false]",b"{invalid}",b"{} tail"] {
            assert!(decode(bytes).is_err(),"{bytes:?}");
        }
        assert_eq!(decode(&[0xff]),Err(JsonError::InvalidUtf8));
        for n in [b"NaN".as_slice(),b"Infinity",b"[{\"a\":1.2}]"]{
            assert!(decode(n).is_err());
        }
    }
    #[test]
    fn comp_05_encode_ascii_and_semantic_golden(){
        let record=row(BTreeMap::from([
            ("cell_id".into(),Datum::String("A".into())),
            ("double_metric".into(),Datum::Signed(24)),
        ]));
        let output=encode_array(&[record]).unwrap();
        assert_eq!(std::str::from_utf8(&output).unwrap(),r#"[{"cell_id":"A","double_metric":24}]"#);
        let unicode=row(BTreeMap::from([("text".into(),Datum::String("💡é".into()))]));
        assert_eq!(std::str::from_utf8(&encode_array(&[unicode]).unwrap()).unwrap(),r#"[{"text":"\ud83d\udca1\u00e9"}]"#);
        assert_eq!(encode_array(&[]),Ok(b"[]".to_vec()));
    }
    #[test]
    fn comp_06_u64_bigint_and_unsupported_wire_types(){
        let tree=decode(br#"{"a":18446744073709551615,"b":18446744073709551616,"c":null}"#).unwrap();
        assert_eq!(tree[0]["a"],Datum::Unsigned(u64::MAX));
        assert_eq!(tree[0]["b"],Datum::bigint("18446744073709551616").unwrap());
        assert_eq!(tree[0]["c"],Datum::Null);
        assert!(tree[0].get("missing").is_none());
        let written=encode_one(&tree[0]).unwrap();
        assert!(std::str::from_utf8(&written).unwrap().contains("18446744073709551616"));
        assert_eq!(encode_one(&BTreeMap::from([("bytes".into(),Datum::Bytes(vec![0xff]))])),Err(JsonError::UnsupportedType));
    }
    #[test]
    fn comp_f01_malformed_surrogates_escapes_duplicate_keys_and_depth(){
        for bytes in [
            br#"{"a": "\uD800"}"#.as_slice(),
            br#"{"a": "\uDC00"}"#,
            br#"{"a": "\uD800\u0000"}"#,
            br#"{"a": "\q"}"#,
            br#"{"a": 01}"#,
            br#"{"a": -}"#,
            br#"{"a": 1, "a": 2}"#,
        ] { assert!(decode(bytes).is_err(),"{bytes:?}"); }
        let deep=format!("{{\"a\":{}}}", "[".repeat(33)+"0"+&"]".repeat(33));
        assert_eq!(decode(deep.as_bytes()),Err(JsonError::ResourceExhausted));
    }
    #[test]
    fn res_01_03_input_records_and_encoded_growth_are_bounded(){
        assert_eq!(decode(&vec![b' ';RAW_FILE_MAX+1]),Err(JsonError::ResourceExhausted));
        let too_many=format!("[{}]",std::iter::repeat_n("{}",RECORDS_PER_SOURCE_MAX+1).collect::<Vec<_>>().join(","));
        assert_eq!(decode(too_many.as_bytes()),Err(JsonError::ResourceExhausted));
        let large=row(BTreeMap::from([("x".into(),Datum::String("a".repeat(ENCODED_RECORD_MAX)))]));
        assert_eq!(encode_array(&[large]),Err(JsonError::ResourceExhausted));
    }
}