// Basic Architecture
// Class BmLite
// function DeleteAll
// function Enroll
// function Verify

//#![deny(missing_docs)]
//#![deny(warnings)]
#![feature(unsize)]
#![no_std]

#![feature(alloc)]
// Plug in the allocator crate

extern crate alloc;
use alloc::vec::Vec;

extern crate embedded_hal;
extern crate crc;
extern crate byteorder;

#[macro_use(block)]

extern crate nb;

use embedded_hal::digital::{InputPin,OutputPin};
use embedded_hal::blocking::spi::Transfer;
use embedded_hal::spi::FullDuplex;
use byteorder::{ByteOrder,BigEndian};

pub struct BmLite<SPI,CS,RST,IRQ> {
	spi: SPI,
	cs: CS,
    rst: RST,
    irq: IRQ,
}

pub enum Error<E>{
    UnexpectedResponse,
    Timeout,
    HalErr(E),
}



impl <SPI,CS,RST,IRQ, E> BmLite<SPI,CS,RST,IRQ>
where  SPI: Transfer<u8, Error = E>,
    SPI: FullDuplex<u8, Error = E>,
	CS: OutputPin,
    RST: OutputPin,
    IRQ: InputPin
{
	    /// Creates a new driver from an SPI peripheral and a chip select
    /// digital I/O pin.
    pub fn new(spi: SPI, cs: CS, rst: RST, irq: IRQ) -> Self {
        let en= BmLite { spi: spi, cs: cs, rst: rst, irq: irq };

        en
    }
	pub fn reset(&mut self) -> Result<u8, Error<E>>  {
        self.rst.set_low();
        //ToDo add a delay here.
        self.rst.set_high();
        //ToDoReset internal data strutures when they exist
        Ok(0)
    }
	pub fn delete_all(&mut self) -> Result<u8, Error<E>>  {
		use crc::*;
		//  [0x01,0x00,0x12,0x00]
		// CRC is caclulated over transport layer bytes only but little endian(sic!)
		// , 0xb1, 0x2e, 0x45, 0x93
		// 0x93,0x45,0x2e,0xb1
		let mut transport:Vec<u8> = [0,0,0,0,0x0c,0x00,0x01,0x00,0x01,0x00,0x02,0x40,0x02,0x00,0x09,0x10,0x00,0x00,0x07,0x00,0x00,0x00].to_vec();

		let crc = crc32::checksum_ieee(&transport[4..]);
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

        self.cs.set_low();
        let _ans = self.spi.transfer( &mut transport).map_err(Error::HalErr)?;
        self.cs.set_high();

        while self.irq.is_low(){
        }
        self.cs.set_low();
        let mut ack:Vec<u8> = [0,0,0,0].to_vec();
        let ack = self.spi.transfer(&mut ack).map_err(Error::HalErr)?;
        self.cs.set_high();

        // expect magic 7f ff 01 7f
        if ! (ack[0] == 0x7f && ack[1] == 0xff && ack[2] == 0x01 && ack[3] == 0x7f ) {
            return Err(Error::UnexpectedResponse)
        }
        while self.irq.is_low(){
        }
        self.cs.set_low();
        let mut v0:Vec<u8> = [0,0,0,0].to_vec();
        let v0 = self.spi.transfer(&mut v0).map_err(Error::HalErr)?;
        self.cs.set_high();
        if ! (v0[0] == 0 && v0[1] == 0xf && v0[2] == 0x01 && v0[3] == 0x7f ) {
         //handle error here should be 0 0 size 0 or something
            //return Err(());
        }
        let xxx:usize = 4 + v0[2] as usize;
        let mut v:Vec<u8> = Vec::with_capacity(xxx);
        self.cs.set_low();
        for _i in 0..xxx {
           let _ans=block!(self.spi.send(0)).map_err(Error::HalErr)?;
           let ans:u8 = block!(self.spi.read()).map_err(Error::HalErr)?;
           v.push(ans);
        }
        self.cs.set_high();
		let crc = crc32::checksum_ieee(&v[0..xxx-4]);

        if crc == BigEndian::read_u32(&v[xxx-4..xxx]){
            self.cs.set_low();
            let mut ack:Vec<u8> = [0x7f,0xff,0x01,0x7f].to_vec();
            let mut ack = self.spi.transfer(&mut ack).map_err(Error::HalErr)?;
            self.cs.set_high();
            return Ok(1);
        }else {
            //crc error
            return Ok(99);
        }

        Ok(0)
	}
}

/*
fn main() {
	let mut dummy = DummyInterface::new();
	let mut encoder = BmLite{internaldata: 0, myio: dummy};
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
*/
#[cfg(test)]
impl DummyInterface{
	fn new() -> Self{
		DummyInterface {
		readdatav: vec!(),writedatav: vec!() }
	}
}
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


		let mut encoder = BmLite{internaldata: 0, myio: dummy};
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
