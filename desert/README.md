# desert

traits for {de,}serializing compact binary formats

* compact representations for the builtin container types (tuples, arrays,
  slices, vectors) is provided.
* varint length encoding scheme for slices and vectors (1 byte for len < 128)
* emphasis on ergonomics of implementing custom binary {de,}serializers

This crate does not (presently) provide automatic derivation. Instead, the
emphasis is on having a good collection of compact implementations for built-in
containers and making it easy to implement your own binary formats manually.

# example

You can use the methods on built-in types and containers:

```rust
use desert::{ToBytes,ToBytesBE,ToBytesLE,FromBytesBE,CountBytes};
type Error = Box<dyn std::error::Error+Send+Sync>;

fn main() -> Result<(),Error> {
  // no overhead for tuples and arrays
  assert_eq![(5u16,6u16,7u16).to_bytes_be()?, vec![0,5,0,6,0,7]];
  assert_eq![[5u16,6u16,7u16].to_bytes_be()?, vec![0,5,0,6,0,7]];

  // minimal overhead for slices and vectors using varints
  assert_eq![
    vec![100u16,101u16,102u16].as_slice().to_bytes_be()?,
    vec![6,0,100,0,101,0,102]
  ];
  assert_eq![vec![0u8;500].count_bytes(), 502];

  // without endianness defaults to big-endian
  assert_eq![(5u16,6u16).to_bytes()?, vec![0,5,0,6]];
  assert_eq![(5u16,6u16).to_bytes_be()?, vec![0,5,0,6]];
  assert_eq![(5u16,6u16).to_bytes_le()?, vec![5,0,6,0]];

  // construct an array from bytes and get the size of bytes read
  assert_eq![
    <[u16;2]>::from_bytes_be(&vec![0,5,0,6])?,
    (4,[5u16,6u16])
  ];
  // this way you can load data structures from slices with extra at the end
  assert_eq![
    <[u16;2]>::from_bytes_be(&vec![0,5,0,6,7,8,9,10,11])?,
    (4,[5u16,6u16])
  ];

  // count how many bytes will need to be read for this Vec<u16>
  assert_eq![
    <Vec<u16>>::count_from_bytes(&vec![6,0,100,0,101,0,102,55,44,33,22,11])?,
    7
  ];

  Ok(())
}
```

And you can define your own types:

```rust
use desert::{ToBytes,FromBytes};
type Error = Box<dyn std::error::Error+Send+Sync>;

#[derive(Debug)]
enum Item { A((f32,f32)), B(u32) }

#[derive(Debug)]
struct Custom { foo: u64, items: Vec<Item> }

impl ToBytes for Custom {
  fn to_bytes(&self) -> Result<Vec<u8>,Error> {
    let mut bytes = vec![];
    // Store foo (in big endian).
    bytes.extend(&self.foo.to_bytes()?);

    // Store the number of items (in big endian).
    bytes.extend(&(self.items.len() as u16).to_bytes()?);

    // Use a bitfield to more compactly represent
    // whether an Item is an A or B.
    let mut bitfield = vec![0u8;(self.items.len()+7)/8];
    for (i,item) in self.items.iter().enumerate() {
      bitfield[i/8] |= match item {
        Item::A(_) => 0,
        Item::B(_) => 1,
      } << (i%8);
    }
    bytes.extend(bitfield);

    // Write out each item serially.
    for item in self.items.iter() {
      bytes.extend(match item {
        Item::A(x) => x.to_bytes()?,
        Item::B(x) => x.to_bytes()?
      });
    }
    Ok(bytes)
  }
}

impl FromBytes for Custom {
  fn from_bytes(src: &[u8]) -> Result<(usize,Self),Error> {
    let mut offset = 0;

    // Read foo (in big endian).
    let (size,foo) = u64::from_bytes(&src[offset..])?;
    offset += size;

    // Read the number of items (in big endian).
    let (size,item_len) = u16::from_bytes(&src[offset..])?;
    offset += size;

    // Read the bitfield data but keep it as a u8 slice.
    let bitfield_len = ((item_len+7)/8) as usize;
    let bitfield = &src[offset..offset+bitfield_len];
    offset += bitfield_len;

    // Read the items, checking the bitfield to know whether an item
    // is an A or a B.
    let mut items = vec![];
    for i in 0..item_len as usize {
      if (bitfield[i/8]>>(i%8))&1 == 0 {
        let (size,x) = <(f32,f32)>::from_bytes(&src[offset..])?;
        items.push(Item::A(x));
        offset += size;
      } else {
        let (size,x) = u32::from_bytes(&src[offset..])?;
        items.push(Item::B(x));
        offset += size;
      }
    }
    Ok((offset, Custom { foo, items }))
  }
}

fn main() -> Result<(),Error> {
  let bytes = Custom {
    foo: 1234567890123456789,
    items: vec![
      Item::A((3.0,4.2)),
      Item::B(1337),
      Item::A((5.5,6.6))
    ]
  }.to_bytes()?;
  println!["serialized: {:?}", bytes];

  let (size,custom) = Custom::from_bytes(&bytes)?;
  println!["deserialized {} bytes: {:?}", size, custom];
  Ok(())
}
```

# rationale

Rust core has some useful methods defined on builtin types:
`5000u32.to_be_bytes()`, `u32::from_be_bytes([u8;4])`, etc.

These methods are certainly useful, but they belong to the builtin types, not
traits. This makes it difficult to write a generic interface that accepts
anything that can be serialized to bytes.

Other options such as serde with bincode let you derive custom implementations
of Serialize and Derive, which are very convenient and you can support for many
other output formats besides binary. However, if you want implement your own
custom serialization and especially deserialization, things get very difficult.
For deserialization you need to implement a Visitor and things get very messy
quickly.

bincode also makes trade-offs that make sense for quickly marshalling data into
and out of memory with minimal parsing overhead, but this choice means that
things like vectors get `usize` bytes padded to the beginning which is 8 whole
bytes on a 64-bit machine and enums under the automatic derivation will always
be prefaced with a `u32` even if there are only 2 options to enumerate. These
trade-offs are not ideal for situations where you need more fine-grained control
over the representation of your structs at a byte-level to keep overhead low or
to integrate with an existing externally-defined wire-protocol. But to achieve
those custom byte formats with serde and bincode, you would need to implement
very generic and difficult serde interfaces when you might only care about bytes
over the wire or on disk.

Another common issue working with binary data is reading large chunks from the
network or from disk in sizes determined by the network transport or physical
medium. Those sizes are very unlikely to map nicely to your data structures, so
it is very useful to be able to count (without parsing into a new instance) how
many bytes can be read from a `[u8]` slice to arrive at the ending byte offset
for the particular data structure under consideration which may have a dynamic
size. Or it may also be helpful to know when the data structure's representation
extends past the end of the buffer slice and the program should fetch more data
from the network or from on disk. These concerns are provided by the
`CountBytes` traits.

# api

[Read the full documentation](https://docs.rs/desert)

This crate consists of these traits for handling binary data:

``` rust
type Error = Box<dyn std::error::Error+Send+Sync>;

pub trait ToBytes {
  fn to_bytes(&self) -> Result<Vec<u8>,Error>;
  fn write_bytes(&self, dst: &mut [u8]) -> Result<usize,Error>;
}
pub trait ToBytesBE {
  fn to_bytes_be(&self) -> Result<Vec<u8>,Error>;
  fn write_bytes_be(&self, dst: &mut [u8]) -> Result<usize,Error>;
}
pub trait ToBytesLE {
  fn to_bytes_le(&self) -> Result<Vec<u8>,Error>;
  fn write_bytes_le(&self, dst: &mut [u8]) -> Result<usize,Error>;
}

pub trait FromBytes: Sized {
  fn from_bytes(src: &[u8]) -> Result<(usize,Self),Error>;
}
pub trait FromBytesBE: Sized {
  fn from_bytes_be(src: &[u8]) -> Result<(usize,Self),Error>;
}
pub trait FromBytesLE: Sized {
  fn from_bytes_le(src: &[u8]) -> Result<(usize,Self),Error>;
}

pub trait CountBytes {
  fn count_from_bytes(buf: &[u8]) -> Result<usize,Error>;
  fn count_from_bytes_more(buf: &[u8]) -> Result<Option<usize>,Error>;
  fn count_bytes(&self) -> usize;
}
pub trait CountBytesBE {
  fn count_from_bytes_be(buf: &[u8]) -> Result<usize,Error>;
  fn count_from_bytes_be_more(buf: &[u8]) -> Result<Option<usize>,Error>;
  fn count_bytes_be(&self) -> usize;
}
pub trait CountBytesLE {
  fn count_from_bytes_le(buf: &[u8]) -> Result<usize,Error>;
  fn count_from_bytes_le_more(buf: &[u8]) -> Result<Option<usize>,Error>;
  fn count_bytes_le(&self) -> usize;
}
```

As well as implementations for:

* u8, u16, u32, u64, u128
* i8, i16, i32, i64, i128
* f32, f64, bool
* tuples (up to 12 elements)
* arrays, vectors, and slices

# license

MIT OR Apache-2.0
