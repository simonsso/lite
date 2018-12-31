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
use crc::crc32;
extern crate byteorder;

extern crate nb;

use embedded_hal::digital::{InputPin,OutputPin};
use embedded_hal::blocking::spi::Transfer;

use byteorder::{ByteOrder,LittleEndian};
trait TransportBuffer<Output>{
    fn create_transport_buffer() -> Output;
    fn push_crc(self) ->Self;
    fn set_cmd(self,u16) ->Self;
    fn get_cmd(&self) -> Option<u16>;
    fn push_u16(self,u16) ->Self;
    fn push_u32(self,u32) ->Self;
    fn add_arg(self,u16) ->Self;
    fn add_arg_u8(self,u16,u8) ->Self;
    fn add_arg_u16(self,u16,u16) ->Self;
    fn add_arg_u32(self,u16,u32) ->Self;
}


impl TransportBuffer<Vec<u8>> for Vec<u8>
    {
    fn create_transport_buffer() -> Vec<u8>{
        let mut v=Vec::with_capacity(256);
        v.extend( &[1,0, 0,0, 0x0,0x00, 0x01,0x00,0x01,0]);
        v
    }
    fn get_cmd(&self) -> Option<u16> {
        if self.len()>=12{
            let resp =  (LittleEndian::read_uint(&self[11..12],2) & 0xFFFF )as u16;
            return Some(resp)
        }
        None
    }
    fn set_cmd(self,cmd:u16) ->Self{
        if self.len()!=10{
            assert!(false,"unexpected command added");
            //self.push or correct code
        }
        self.push_u16(cmd).push_u16(0)
    }
    fn push_crc(self) -> Self{
        let crc = crc::crc32::checksum_ieee(&self[4..]);
		self.push_u32(crc)
    }

    fn push_u16(mut self,data:u16) ->Self{
		self.push((0xFF& data )as u8);
        let data = data /256;
		self.push((0xFF& data )as u8);
        self
    }
    fn push_u32(mut self,data:u32) -> Self{
		self.push((0xFF& data )as u8);
		let data  = data/256;
		self.push((0xFF& data )as u8);
		let data  = data/256;
		self.push((0xFF& data )as u8);
		let data  = data/256;
		self.push((0xFF& data )as u8);
        self
    }
    fn add_arg(mut self,arg:u16) ->Self{
        self[12] +=1 ;
		self.push_u16(arg).push_u16(0)
    }
    fn add_arg_u8(mut self,arg:u16,data:u8) ->Self{
        self[12] +=1 ;
		let mut s = self.push_u16(arg).push_u16(2);
        s.push(data);
        s
    }
    fn add_arg_u16(mut self,arg:u16,data:u16) ->Self{
        self[12] +=1 ;
		self.push_u16(arg).push_u16(2).push_u16(data)
    }
    fn add_arg_u32(mut self,arg:u16,data:u32) -> Self{
        self[12] +=1 ;
		self.push_u16(arg).push_u16(4).push_u32(data)
    }
}

trait LinkBuffer{
    fn parse_result<Closure,E>(&self,u16, f:Closure  ) ->Result<(), Error<E>>
    where Closure:FnMut(u16,&[u8],usize);
}

impl LinkBuffer for Vec<u8>{
   fn parse_result<Closure,E>(&self,cmd:u16,mut callback:Closure  ) ->Result<(), Error<E>>
    
    where Closure:FnMut(u16,&[u8],usize),
    {
   // Parse result args
        let len = self.len();
        if len <6 {
             // expect at lease some data here
             return Err(Error::UnexpectedResponse)
        }
        if cmd != as_u16(self[1],self[0]){
             // command response did not match command.
             return Err(Error::UnexpectedResponse)
        }
        let argc = as_u16(self[3],self[2]);
        let mut current:usize = 4;

        for _i in 0..argc{
            if len < current+4 {
                // Parse error
                return Err(Error::UnexpectedResponse)
            }
            let arg = as_u16(self[1+current],self[current]) ;
            let arglen = as_u16(self[3+current],self[2+current]) as usize ;
            current +=4;
            if len < current+arglen {
                // Parse error
                return Err(Error::UnexpectedResponse)
            }
            callback(arg,&self[current..current+arglen],arglen as usize);
            current +=arglen; 
        }
        Ok(())
    }
}

pub struct BmLite<SPI,CS,RST,IRQ> {
	spi: SPI,
	cs: CS,
    rst: RST,
    irq: IRQ,
    enrolledfingers: u16,
}

pub enum Error<E>{
    UnexpectedResponse,
    Timeout,
    CRCError,
    NoMatch,
    HalErr(E),
}

const    ARG_RESULT:u16 =  0x2001;
const    ARG_COUNT:u16 =   0x2002;
const    _ARG_TIMEOUT:u16 = 0x5001;
const    _ARG_VERSION:u16 = 0x6004;
const    ARG_MATCH:u16 =   0x000A;
const    ARG_ID:u16 =      0x0006;

fn as_u16(h:u8,l:u8) -> u16{
    ((h as u16)<<8)|(l as u16)
}


impl <SPI,CS,RST,IRQ, E> BmLite<SPI,CS,RST,IRQ>
where  SPI: Transfer<u8, Error = E>,
	CS: OutputPin,
    RST: OutputPin,
    IRQ: InputPin
{
	    /// Creates a new driver from an SPI peripheral and a chip select
    /// digital I/O pin.
    pub fn new(spi: SPI, cs: CS, rst: RST, irq: IRQ) -> Self {
        let en= BmLite { spi: spi, cs: cs, rst: rst, irq: irq , enrolledfingers : 0 };

        en
    }

    pub fn teardown(self) -> (SPI, (CS,RST,IRQ)) {
        // Return interfaces 
        (self.spi,(self.cs,self.rst,self.irq))
    }


	pub fn reset<DelayClass>(&mut self,mut d: DelayClass) -> Result<u8, Error<E>> 
        where DelayClass: FnMut()
    {
 
        self.rst.set_low();
        d();
        self.rst.set_high();
        Ok(0)
    }

    fn link(&mut self, mut transport:Vec<u8> ) ->  Result<(Vec<u8>), Error<E>> {
        let len = transport.len() as u32 -10 ;
		transport[2]=(len & 0xFF) as u8 +6 ; // Size
		transport[3]=0x0; // MSB always 0
		transport[4]=(len & 0xFF) as u8  ;   // Size
		transport[5]=0x0; // MSB always 0

        transport = transport.push_crc();

        self.cs.set_low();
        let _ans = self.spi.transfer( &mut transport).map_err(Error::HalErr)?;
        self.cs.set_high();

        let mut timeout:i32 = 500_000;
        while self.irq.is_low(){
            timeout -=1;
            if timeout < 0 {
                return Err(Error::Timeout)
            }
        }
        self.cs.set_low();
        let mut ack:Vec<u8> = [0,0,0,0].to_vec();
        let ack = self.spi.transfer(&mut ack).map_err(Error::HalErr)?;
        self.cs.set_high();

        // expect magic 7f ff 01 7f
        if ! (ack[0] == 0x7f && ack[1] == 0xff && ack[2] == 0x01 && ack[3] == 0x7f ) {
            return Err(Error::UnexpectedResponse)
        }
        //timeout = 500_000;
        while self.irq.is_low(){
         //   timeout -=1;
         //   if timeout < 0 {
         //       return Err(Error::Timeout)
         //   }
        }
        self.cs.set_low();
        let mut v0:Vec<u8> = [0,0,0,0].to_vec();
        let v0 = self.spi.transfer(&mut v0).map_err(Error::HalErr)?;
        self.cs.set_high();

        let transportsize:usize = 4 + v0[2] as usize;
        let mut v:Vec<u8> = Vec::with_capacity(transportsize);
        self.cs.set_low();
        for _i in 0..transportsize {
           v.push(0);
        }
        {   // Scope off mutable borrow of v
            // internal array of v is returned in _ans but updated data still pressent
            // in vector v when the mutable borrow ends.
            let _ans = self.spi.transfer( &mut v).map_err(Error::HalErr)?;
        }
        self.cs.set_high();

		let crc = crc32::checksum_ieee(&v[0..transportsize-4]);

        if crc == LittleEndian::read_u32(&v[transportsize-4..transportsize]){
            self.cs.set_low();
            let mut ack = [0x7f,0xff,0x01,0x7f];
            let _ack = self.spi.transfer(&mut ack).map_err(Error::HalErr)?;
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

	pub fn get_version(&mut self) -> Result<u8, Error<E>>  {
        let cmd = 0x3004;
        let transport = <Vec<u8> as TransportBuffer<Vec<u8>>>::create_transport_buffer().set_cmd(cmd).add_arg(0x1004);
        let resp=self.link(transport)?;

        // handle all responses here
        let mut ok_resp = false;
        resp.parse_result(cmd, |arg,_argv,_arglen|{
            match arg {
               ARG_RESULT => {ok_resp = true}
            //   ARG_MATCH  => { litematch = (LittleEndian::read_uint(&argv,arglen) & 0xFFFF_FFFF )as u32; }
            //   ARG_ID  => { remaining = (LittleEndian::read_uint(&argv,arglen) & 0xFFFF_FFFF )as u32; }
                _other => {} // For args we do not care about
            }
        })?;
        if ! ok_resp {
             return Err(Error::UnexpectedResponse)
        }
        Ok(0)
    }
    // Timeout in ms but 0 waits forever
	pub fn capture(&mut self, timeout:u32) -> Result<u8, Error<E>>  {
        let cmd = 0x0001;
        let mut transport = <Vec<u8> as TransportBuffer<Vec<u8>>>::create_transport_buffer().set_cmd(cmd);
        if timeout != 0 {
            transport = transport.add_arg_u32(0x5001, timeout );
        }
        let resp=self.link(transport)?;

        let mut captureresult = 0;
        let mut ok_resp = false;
        resp.parse_result(cmd, |arg,argv,arglen|{
            match arg {
               ARG_RESULT => {ok_resp = true;
                              captureresult = (LittleEndian::read_uint(&argv,arglen) & 0xFFFF_FFFF )as u32;
                            }
                _other => {} // For args we do not care about
            }
        })?;
        if ok_resp  {
            return Ok(captureresult as _);
        }
        Err(Error::UnexpectedResponse)
    }
	pub fn enroll<F>(&mut self, mut f:F ) -> Result<u32, Error<E>>
        where F:FnMut(u32)
    {
        let mut enrolling = 100;
        f(enrolling as u32);
        self.do_enroll(0x03)?; //begin
        while enrolling > 0{
            f(enrolling as u32);
            self.waitfingerup(0)?;
            self.capture(0)?;
            enrolling = self.do_enroll(0x04)?; //add image
        }
        self.do_enroll(0x05)?; //done
        let e = self.enrolledfingers;
        self.do_savetemplate(e)?;
        self.enrolledfingers += 1;
        Ok(0)
    }

	pub fn do_enroll(&mut self, state:u16) -> Result<u32, Error<E>>  {
        let cmd = 0x0002;
        let mut transport = <Vec<u8> as TransportBuffer<Vec<u8>>>::create_transport_buffer().set_cmd(cmd);
        if state != 0 {
            transport=transport.add_arg(state);
        }
        let resp=self.link(transport)?;
        // handle all responses here
        let mut remaining:u32 = 0;
        let mut ok_resp = false;
        resp.parse_result(cmd, |arg,argv,arglen|{
            match arg {
               ARG_RESULT => {ok_resp = true}
               ARG_COUNT  => { remaining = (LittleEndian::read_uint(&argv,arglen) & 0xFFFF_FFFF )as u32; }
                _other => {} // For args we do not care about
            }
        })?;
        if ok_resp {
             return Ok(remaining);
        }
        Err(Error::UnexpectedResponse)
    }

	pub fn do_savetemplate(&mut self , tplid:u16 ) -> Result<u32, Error<E>>  {
        let cmd = 0x0006;
        let transport = <Vec<u8> as TransportBuffer<Vec<u8>>>::create_transport_buffer().set_cmd(cmd).add_arg(0x1008).add_arg_u16(0x0006,tplid);
        let resp=self.link(transport)?;
        // handle all responses here
        let mut ok_resp = false;
        resp.parse_result(cmd, |arg,_argv,_arglen|{
            match arg {
               ARG_RESULT => {ok_resp = true}
                _other => {} // For args we do not care about
            }
        })?;
        if ok_resp {
            return Ok(0);
        }
        Err(Error::UnexpectedResponse)
    }


	pub fn do_extract(&mut self) -> Result<u32, Error<E>>  {
        let cmd = 0x0005;
        let transport = <Vec<u8> as TransportBuffer<Vec<u8>>>::create_transport_buffer().set_cmd(cmd).add_arg(0x0008);

        let resp=self.link(transport)?;

        // handle all responses here
        let mut remaining:u32 = 0;
        let mut ok_resp = false;

        resp.parse_result(cmd, |arg,argv,arglen|{
            match arg {
               ARG_RESULT => {ok_resp = true}
               ARG_COUNT  => { remaining = (LittleEndian::read_uint(&argv,arglen) & 0xFFFF_FFFF )as u32; }
               _other => {} // For args we do not care about
            }
        })?;
        if ok_resp {
            return Ok(remaining);
        }
        Err(Error::UnexpectedResponse)
    }

	pub fn identify(&mut self) -> Result<u32, Error<E>>  {
        self.capture(0)?;
        self.do_extract()?;
        self.do_identify()
    }
	pub fn do_identify(&mut self) -> Result<u32, Error<E>>  {
        let cmd = 0x0003;
        let transport = <Vec<u8> as TransportBuffer<Vec<u8>>>::create_transport_buffer().set_cmd(cmd);

        let resp=self.link(transport)?;
        // handle all responses here
        let mut remaining = 0xFFFF_FFFF;
        let mut litematch:u32 = 0;
        let mut ok_resp = false;
        resp.parse_result(cmd, |arg,argv,arglen|{
            match arg {
               ARG_RESULT => {ok_resp = true}
               ARG_MATCH  => { litematch = (LittleEndian::read_uint(&argv,arglen) & 0xFFFF_FFFF )as u32; }
               ARG_ID  => { remaining = (LittleEndian::read_uint(&argv,arglen) & 0xFFFF_FFFF )as u32; }
                _other => {} // For args we do not care about
            }
        })?;
        if litematch == 0 {
            return Err(Error::NoMatch);
        }
        if ok_resp && litematch != 0 {
            return Ok(remaining);
        }
        Err(Error::UnexpectedResponse)
    }



	pub fn waitfingerup(&mut self, timeout:u32) -> Result<u8, Error<E>>  {
        let cmd = 0x007;
        let mut transport = <Vec<u8> as TransportBuffer<Vec<u8>>>::create_transport_buffer().set_cmd(cmd);
        if timeout != 0 {
            transport=transport.add_arg_u32(0x5001,timeout);
        }
        transport = transport.add_arg(0x0002); //0002 Enroll

        let resp=self.link(transport)?;
        let mut fingerpresent = 0;
        let mut ok_resp = false;
        resp.parse_result(cmd, |arg,argv,arglen|{
            match arg {
               ARG_RESULT => {ok_resp = true;
                              fingerpresent = (LittleEndian::read_uint(&argv,arglen) & 0xFFFF_FFFF )as u32;
                            }
                _other => {} // For args we do not care about
            }
        })?;
        if ok_resp  {
            return Ok(fingerpresent as _);
        }
        Err(Error::UnexpectedResponse)
    }
	pub fn delete_all(&mut self) -> Result<u8, Error<E>>  {
        let cmd = 0x4002;
        let transport = <Vec<u8> as TransportBuffer<Vec<u8>>>::create_transport_buffer().set_cmd(cmd).add_arg(0x1009).add_arg(0x0007);
        let resp=self.link(transport)?;

        let mut deleteallresult = 0;
        let mut ok_resp = false;
        resp.parse_result(cmd, |arg,argv,arglen|{
            match arg {
               ARG_RESULT => {ok_resp = true;
                              deleteallresult = (LittleEndian::read_uint(&argv,arglen) & 0xFFFF_FFFF )as u32;
                            }
                _other => {} // For args we do not care about
            }
        })?;
        if ok_resp  {
            return Ok(deleteallresult as _);
        }
        Err(Error::UnexpectedResponse)	
    }
}

#[cfg(test)]
mod tests {
use tests::std::cell::RefCell;

struct DummyInterface {
	data:  RefCell<Vec<bool>>,
}
impl DummyInterface{
    fn new(l:Vec<bool>)-> Self{
        DummyInterface{ data:RefCell::new(l) }
        }
}
impl super::OutputPin for DummyInterface {
	fn set_low(&mut self ) {
	    self.data.borrow_mut().push(false)	
	}
	fn set_high(&mut self) {
	    self.data.borrow_mut().push(true)	
	}
}

impl super::InputPin for DummyInterface {
	fn is_high(&self ) -> bool { 
	    self.data.borrow_mut().pop().unwrap()
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
	fn capture_identify() {
		use super::*;
        let expectations = [
            SpiTransaction::transfer([0x01,0x00,0x0a,0x00,0x04,0x00,0x01,0x00,0x01,0x00,0x01,0x00,0x00,0x00,0x52,0x7c,0x2b,0x55,].to_vec(),[0;18].to_vec()),
            SpiTransaction::transfer([0,0,0,0].to_vec(),[0x7f,0xff,0x01,0x7f].to_vec()),
            SpiTransaction::transfer([0,0,0,0].to_vec() ,[0,0,17-2,0].to_vec()),
            // CRC 2418401667 9025e183 over 15 bytes

            SpiTransaction::transfer([0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,].to_vec(),[0x09,0x00,0x01,0x00,0x01,0x00,0x01,0x00,0x01,0x00,0x01,0x20,0x01,0x00,0x00,0x83,0xe1,0x25,0x90,].to_vec()),
            SpiTransaction::transfer([0x7f,0xff,0x01,0x7f].to_vec(),[0,0,0,0].to_vec()),
            SpiTransaction::transfer([0x01,0x00,0x0e,0x00,0x08,0x00,0x01,0x00,0x01,0x00,0x05,0x00,0x01,0x00,0x08,0x00,0x00,0x00,0x8e,0xb5,0x8d,0xd0,].to_vec(),[0;22].to_vec()),
            SpiTransaction::transfer([0,0,0,0].to_vec(),[0x7f,0xff,0x01,0x7f].to_vec()),
            SpiTransaction::transfer([0,0,0,0].to_vec() ,[0,0,17-2,0].to_vec()),
            // CRC 3452547215 cdc9b08f over 15 bytes

            SpiTransaction::transfer([0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,].to_vec(),[0x09,0x00,0x01,0x00,0x01,0x00,0x05,0x00,0x01,0x00,0x01,0x20,0x01,0x00,0x00,0x8f,0xb0,0xc9,0xcd,].to_vec()),
            SpiTransaction::transfer([0x7f,0xff,0x01,0x7f].to_vec(),[0,0,0,0].to_vec()),
            SpiTransaction::transfer([0x01,0x00,0x0a,0x00,0x04,0x00,0x01,0x00,0x01,0x00,0x03,0x00,0x00,0x00,0xd9,0xb4,0x22,0xff,].to_vec(),[0;18].to_vec()),
            SpiTransaction::transfer([0,0,0,0].to_vec(),[0x7f,0xff,0x01,0x7f].to_vec()),
            SpiTransaction::transfer([0,0,0,0].to_vec() ,[0,0,28-2,0].to_vec()),
            // CRC 4072009766 f2b5f026 over 26 bytes

            SpiTransaction::transfer([0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,].to_vec(),[0x14,0x00,0x01,0x00,0x01,0x00,0x03,0x00,0x03,0x00,0x0a,0x00,0x01,0x00,0x01,0x06,0x00,0x02,0x00,0x01,0x00,0x01,0x20,0x01,0x00,0x00,0x26,0xf0,0xb5,0xf2,].to_vec()),
            SpiTransaction::transfer([0x7f,0xff,0x01,0x7f].to_vec(),[0,0,0,0].to_vec()),

        ];

        let spi = SpiMock::new(&expectations);

        let dummy_cs = DummyInterface::new([false,false,false].to_vec());
        let dummy_irq = DummyInterface::new([false,true,false,true,false,true,false,true,false,true,false,true,false,true,false,true,false,true,false,true,false,true,false,true].to_vec());
        let dummy_reset = DummyInterface::new([false].to_vec());

		let mut encoder = BmLite::new(spi, dummy_cs,dummy_reset,dummy_irq );
		let ans = encoder.identify();
        match ans {
            Err(_) => {assert!(false, "Function returned unexpected error")}
            Ok(_) => {}
        }

        let (mut spi, (_a,_b,_c)) = encoder.teardown();
        spi.done();
	}

	#[test]
    #[should_panic]
	fn capture_identify_nodata() {
		use super::*;
        let expectations = [
        ];

        let spi = SpiMock::new(&expectations);

        let dummy_cs = DummyInterface::new([false,false,false].to_vec());
        let dummy_irq = DummyInterface::new([false,true,false,true,false,true,false,true,false,true,false,true,false,true,false,true,false,true,false,true,false,true,false,true].to_vec());
        let dummy_reset = DummyInterface::new([false].to_vec());

		let mut encoder = BmLite::new(spi, dummy_cs,dummy_reset,dummy_irq );
		let ans = encoder.identify();
        match ans {
            Err(_x) => {assert!(false, "Function returned unexpected error")}
            Ok(_) => {}
        }

        let (mut spi, (_a,_b,_c)) = encoder.teardown();
        spi.done();
	}

	#[test]
	fn delete_all_templates() {
		use super::*;

		//dummy.readdatav.push(vec!(0,0,01,0));
	    //dummy.readdatav.push(vec!(0x3,0x7f,01,0x7f));
		//dummy.readdatav.push(vec!(0xff,0x7f,01,0x7f));
        // Configure expectations

		let refvec:Vec<u8>   = [0x01,0x00,0x12,0x00,0x0c,0x00,0x01,0x00,0x01,0x00,0x02,0x40,0x02,0x00,0x09,0x10,0x00,0x00,0x07,0x00,0x00,0x00,0xb1,0x2e,0x45,0x93].to_vec();
        let dontcare:Vec<u8> = [0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00].to_vec();

        let expectations = [
            SpiTransaction::transfer(refvec,dontcare),
            SpiTransaction::transfer([0,0,0,0].to_vec(),[0x7f,0xff,0x01,0x7f].to_vec()),
            SpiTransaction::transfer([0,0,0,0].to_vec(),[0x00,0x00,0x0F,0x00].to_vec()),
            SpiTransaction::transfer([0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,].to_vec(),[0x09,0x00,0x01,0x00,0x01,0x00,0x02,0x40,0x01,0x00,0x01,0x20,0x01,0x00,0x00,0xab,0x1f,0x35,0x80,].to_vec()),
            SpiTransaction::transfer([0x7f,0xff,0x01,0x7f].to_vec(),[0,0,0,0].to_vec()),


        ];

        let spi = SpiMock::new(&expectations);

        let dummy_cs = DummyInterface::new([false,false,false].to_vec());
        let dummy_irq = DummyInterface::new([false,true,false,true,false,true,false,true,false,true,false,true].to_vec());
        let dummy_reset = DummyInterface::new([false].to_vec());

		let mut encoder = BmLite::new(spi, dummy_cs,dummy_reset,dummy_irq );
		let ans = encoder.delete_all();
        match ans {
            Err(_x) => {assert!(false, "Function returned unexpected error")}
            Ok(_) => {}
        }

        let (mut spi, (_a,_b,_c)) = encoder.teardown();
        spi.done();
	}

    #[test]
    fn capture_image() {
		use super::*;
        let expectations = [
                SpiTransaction::transfer([0x01,0x00,0x0a,0x00,0x04,0x00,0x01,0x00,0x01,0x00,0x01,0x00,0x00,0x00,0x52,0x7c,0x2b,0x55,].to_vec(),[0;18].to_vec()),
                SpiTransaction::transfer([0,0,0,0].to_vec(),[0x7f,0xff,0x01,0x7f].to_vec()),
                SpiTransaction::transfer([0,0,0,0].to_vec() ,[0,0,17-2,0].to_vec()),
                // CRC 2418401667 9025e183 over 15 bytes

            SpiTransaction::transfer([0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,].to_vec(),[0x09,0x00,0x01,0x00,0x01,0x00,0x01,0x00,0x01,0x00,0x01,0x20,0x01,0x00,0x00,0x83,0xe1,0x25,0x90,].to_vec()),
                SpiTransaction::transfer([0x7f,0xff,0x01,0x7f].to_vec(),[0,0,0,0].to_vec()),
        ];

        let spi = SpiMock::new(&expectations);

        let dummy_cs = DummyInterface::new([false,false,false].to_vec());
        let dummy_irq = DummyInterface::new([false,true,false,true,false,true,false,true,false,true,false,true,false,true,false,true,false,true,false,true,false,true,false,true].to_vec());
        let dummy_reset = DummyInterface::new([false].to_vec());

		let mut encoder = BmLite::new(spi, dummy_cs,dummy_reset,dummy_irq );
		let ans = encoder.capture(0);
        match ans {
            Err(_x) => {assert!(false, "Function returned unexpected error")}
            Ok(ans) => {assert!(ans == 0) }
        }

        let (mut spi, (_a,_b,_c)) = encoder.teardown();
        spi.done();

	}
}
