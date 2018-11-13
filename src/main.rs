// Basic Architecture
// Class Encoder 
// function DeleteAll
// function Enroll
// function Verify

// have an halinterface


const LEN: usize = 32;
pub trait Serialio {
	fn readdata(&self, len:u32) -> ( [u8; LEN] );
	fn writedata(&self,[u8 ; LEN],len:u32) ; 
}

struct Encoder {
	internaldata: i32,
	myio: DummyInterface,
}



impl Encoder {
	fn encode(&self) -> [u8; LEN] {
		self.myio.readdata(4)
	}
}



fn main() {
	let dummy = DummyInterface{};
	let mut encoder = Encoder{internaldata: 0, myio: dummy};
	encoder.encode();
}
struct DummyInterface {
	
}

impl Serialio for DummyInterface {
	fn readdata(&self, len:u32) -> ([u8; LEN]){
		let mut buf =[0xff ; LEN];
		buf[0..4].clone_from_slice(&[0x55,0x55,0x10,0x20]);
		buf
	}
	fn writedata(&self,buffer:[u8; LEN],len:u32){
		
		
	}
	
	
}
#[cfg(test)]
mod tests {
	#[test]
	fn it_works() {
		use super::*;
		let dummy = DummyInterface{};
		let mut encoder = Encoder{internaldata: 0, myio: dummy};
		encoder.encode();
		let dummy = encoder.myio;
	}
	/*
		#[test]
		fn it_works2() {
			let res = super::Encode::encode();
			assert!(res[0] == 1 && res[1] == 2);
		}
    */
}
