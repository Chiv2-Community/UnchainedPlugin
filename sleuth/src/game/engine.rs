#![allow(dead_code)]
#![allow(non_camel_case_types)]
#![allow(clippy::upper_case_acronyms)]

#[repr(C)]
pub enum ENetMode {
	STANDALONE,
	DEDICATED_SERVER,
	LISTEN_SERVER,
	CLIENT,
	MAX
}