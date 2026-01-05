#![allow(dead_code)]

#[repr(C)]
pub enum ENetMode {
	STANDALONE,
	DEDICATED_SERVER,
	LISTEN_SERVER,
	CLIENT,
	MAX
}