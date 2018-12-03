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
use byteorder::{ByteOrder,LittleEndian};

pub struct BmLite<SPI,CS,RST,IRQ> {
	spi: SPI,
	cs: CS,
    rst: RST,
    irq: IRQ,
}

pub enum Error<E>{
    UnexpectedResponse,
    Timeout,
    CRCError,
    HalErr(E),
}

enum SensorResp{
    ARG_Result =  0x2001,
    ARG_Count =   0x2002,
    ARG_Timeout = 0x5001,
    ARG_Version = 0x6004,
}

fn as_u16(h:u8,l:u8) -> u16{
    ((h as u16)<<8)|(l as u16)
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
    fn link(&mut self, appldata:Vec<u8> ) ->  Result<(Vec<u8>), Error<E>> {
		let mut transport:Vec<u8> = [0,0,0,0,0x0c,0x00,0x01,0x00,0x01,0].to_vec();
        transport.extend(appldata.iter());
		use crc::*;
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
        // v is now chanel and size
        // if ! (v0[0] == 0 && v0[1] == 0xf && v0[2] == 0x01 && v0[3] == 0x7f ) {
        //     return Err(Error::UnexpectedResponse)
        //
        // }
        let transportsize:usize = 4 + v0[2] as usize;
        let mut v:Vec<u8> = Vec::with_capacity(transportsize);
        self.cs.set_low();
        for _i in 0..transportsize {
           let _ans=block!(self.spi.send(0)).map_err(Error::HalErr)?;
           let ans:u8 = block!(self.spi.read()).map_err(Error::HalErr)?;
           v.push(ans);
        }
        self.cs.set_high();
		let crc = crc32::checksum_ieee(&v[0..transportsize-4]);

        if crc == LittleEndian::read_u32(&v[transportsize-4..transportsize]){
            self.cs.set_low();
            let mut ack = [0x7f,0xff,0x01,0x7f];
            let mut ack = self.spi.transfer(&mut ack).map_err(Error::HalErr)?;
            self.cs.set_high();
        }else {
            //crc error
            return Err(Error::CRCError)
        }

        // verify sizes v[0] and v[1] -- ignored
         
        // v[2:3] seq num
        // v[4:5] seq len -- for multi frame package this will be where we have reading of multi data

        if (v[2],v[3]) != (v[4],v[5]) {
             // multi packet not expected and supported yet
             return Err(Error::UnexpectedResponse)
        }

        // v[6:7] application package:  (maybe num commands)
        // v[8:9] CMD should be same as CMD sent.
        Ok(v.split_off(6))
    }
	pub fn delete_all(&mut self) -> Result<u8, Error<E>>  {
		let mut transport:Vec<u8> = [0x02, 0x40,0x02,0x00,0x09,0x10,0x00,0x00,0x07,0x00,0x00,0x00].to_vec();
        let cmd = (transport[1],transport[0]);
        let resp=self.link(transport)?;


        if resp.len() <6 {
             // expect at lease some data here
             return Err(Error::UnexpectedResponse)
        }
        if cmd != (resp[1],resp[0]){
             // command response did not match command.
             return Err(Error::UnexpectedResponse)
        }
        // expected data len = 1
        //          Result == ARG_Result
        // val ==1 
        if as_u16(resp[3],resp[2]) != SensorResp::ARG_Result as u16 {
             return Err(Error::UnexpectedResponse)
        }
        let resp_len = as_u16(resp[4],resp[5]);

        Ok(resp[7])
	}
}

/*
fn main() {
	let mut dummy = DummyInterface::new();
	let mut encoder = BmLite{internaldata: 0, myio: dummy};
	encoder.encode();
}
*/
#[cfg(test)]
mod tests {
struct DummyInterface {
	data:  Vec<bool>,
}

impl new for DummyInterface{
    pub fn new() -> Self {
        DummyInterface
    }
    

}
impl super::OutputPin for DummyInterface {
	fn set_low(&mut self ) {
		
	}
	fn set_high(&mut self) {
	   
	}
}

impl super::InputPin for DummyInterface {
	fn is_high(&self ) -> bool {
		self.data.pop()
	}
	fn is_low(&self) -> bool{
	    ! self.is_high()
	}
}


extern crate embedded_hal_mock;
extern crate std;

use tests::embedded_hal_mock::spi::{Mock as SpiMock, Transaction as SpiTransaction};
use tests::std::vec::*;


	#[test]
	fn delete_all_templates() {
		use super::*;

		//dummy.readdatav.push(vec!(0,0,01,0));
	    //dummy.readdatav.push(vec!(0x3,0x7f,01,0x7f));
		//dummy.readdatav.push(vec!(0xff,0x7f,01,0x7f));
        // Configure expectations

		let refvec:Vec<u8> = [0x01,0x00,0x12,0x00,0x0c,0x00,0x01,0x00,0x01,0x00,0x02,0x40,0x02,0x00,0x09,0x10,0x00,0x00,0x07,0x00,0x00,0x00,0xb1,0x2e,0x45,0x93].to_vec();
        let expectations = [
            SpiTransaction::write([2, 2].to_vec()),
            SpiTransaction::transfer([3, 4].to_vec(), refvec),
        ];

        let mut spi = SpiMock::new(&expectations);

        let dummy_cs = DummyInterface::new();
        let dummy_irq = DummyInterface::new();
        let dummy_reset = DummyInterface::new();

		let mut encoder = BmLite{spi, dummy_cs,dummy_reset,dummy_irq };
		encoder.delete_all();
		// assert!( tmp[4..]==refvec[4..]);
		// assert!( &tmp[0..4]==&refvec[0..4]);

		let refvec:Vec<u8> = [0x01,0x00,0x12,0x00,0x0c,0x00,0x01,0x00,0x01,0x00,0x02,0x40,0x02,0x00,0x09,0x10,0x00,0x00,0x07,0x00,0x00,0x00,0xff,0xff,0xff,0xff].to_vec();

        // must implement a teardown first  then call  spi.done();

	}
	/*
		#[test]
		fn it_works2() {
			let res = super::Encode::encode();
			assert!(res[0] == 1 && res[1] == 2);
		}
    */
}
