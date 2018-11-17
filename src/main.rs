// Basic Architecture
// Class Encoder
// function DeleteAll
// function Enroll
// function Verify

// have an halinterface

#![allow(dead_code)]
extern crate crc;

const LEN: usize = 32;
pub trait Serialio {
	fn readdata(&mut self, len:usize) -> ( Vec<u8> );
	fn writedata(&mut self, Vec<u8> ,len:usize) ;
}

struct Encoder {
	internaldata: i32,
	myio: DummyInterface,
}



impl Encoder {
	fn encode(&mut self) -> Vec<u8>  {
		use crc::*;
		//  [0x01,0x00,0x12,0x00]
		// CRC is caclulated over transport layer bytes only but little endian(sic!)
		// , 0xb1, 0x2e, 0x45, 0x93
		// 0x93,0x45,0x2e,0xb1
		let mut transport = vec![0,0,0,0,0x0c,0x00,0x01,0x00,0x01,0x00,0x02,0x40,0x02,0x00,0x09,0x10,0x00,0x00,0x07,0x00,0x00,0x00];
		let crc = crc32::checksum_ieee(&transport[4..]);
		println!("{:x}",crc);

		transport.push((0xFF& crc )as u8);
		let crc  = crc/256;
			transport.push((0xFF& crc )as u8);
		let crc  = crc/256;
			transport.push((0xFF& crc )as u8);
		let crc  = crc/256;
		transport.push((0xFF& crc )as u8);

		//Add linklayer headers
		transport[0]=0x1;
		transport[1]=0x0;
		transport[2]=0x12;
		transport[3]=0x0;

		let	len=transport.len();
		self.myio.writedata(transport,len );
		self.myio.readdata(4)
	}
}


fn main() {
	let mut dummy = DummyInterface::new();
	let mut encoder = Encoder{internaldata: 0, myio: dummy};
	encoder.encode();
}
struct DummyInterface {
	readdatav:  Vec<Vec<u8>>,
	writedatav: Vec<Vec<u8>>,
}

impl Serialio for DummyInterface {
	fn readdata(&mut self, len:usize) -> Vec<u8> {
		let  buf  = self.readdatav.pop();
		let  buf = buf.unwrap();

		assert!( len == buf.len()) ;  // Did dummy provide the expected lenght of data
		buf
	}
	fn writedata(&mut self,buffer:Vec<u8> ,_len:usize){
		self.writedatav.push(buffer)
	}

}
impl DummyInterface{
	fn new() -> Self{
		DummyInterface {
		readdatav: vec!(),writedatav: vec!() }
	}
}

#[cfg(test)]
mod tests {
	#[test]
	fn delete_all_templates() {
		use super::*;
		let vec1=vec![1,2,3];
		let vec2=vec![1,2,3];
		assert!(vec1 == vec2);


		let mut dummy = DummyInterface::new();
		dummy.readdatav.push(vec!(0,0,01,0));
	    dummy.readdatav.push(vec!(0x3,0x7f,01,0x7f));
		dummy.readdatav.push(vec!(0xff,0x7f,01,0x7f));


		let mut encoder = Encoder{internaldata: 0, myio: dummy};
		encoder.encode();
		let mut wb=encoder.myio.writedatav;
		let tmp=wb.pop().unwrap();
		let refvec:Vec<u8> = vec!(0x01,0x00,0x12,0x00,0x0c,0x00,0x01,0x00,0x01,0x00,0x02,0x40,0x02,0x00,0x09,0x10,0x00,0x00,0x07,0x00,0x00,0x00,0xb1,0x2e,0x45,0x93);
		assert!( tmp[4..]==refvec[4..]);
		assert!( &tmp[0..4]==&refvec[0..4]);

		let refvec:Vec<u8> = vec!(0x01,0x00,0x12,0x00,0x0c,0x00,0x01,0x00,0x01,0x00,0x02,0x40,0x02,0x00,0x09,0x10,0x00,0x00,0x07,0x00,0x00,0x00,0xff,0xff,0xff,0xff);

		assert!( &tmp != &refvec)

	}
	/*
		#[test]
		fn it_works2() {
			let res = super::Encode::encode();
			assert!(res[0] == 1 && res[1] == 2);
		}
    */
}
