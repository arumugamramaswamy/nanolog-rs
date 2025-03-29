# Nanolog-rs

A rewrite of [Nanolog](https://github.com/PlatformLab/NanoLog) in rust

This is currently a POC and is not production ready yet, hopefully I get there soon!


# Ideas
- ring buffer shared between reader and writer
	- `create_ring_buffer` call returning a reader and writer
	- writer is clonable but not send 
		- by making the writer not send, we guarantee that only the thread that calls `create_ring_buffer` can write to it
	- reader is not clonable but is send (create the reader and send it back to the main thread)
		- by making the reader not clonable, we guarantee that only one reader can exist at a time
