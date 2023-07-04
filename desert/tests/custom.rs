use desert::{Error, FromBytes, ToBytes};

#[derive(Debug, PartialEq)]
enum Item {
    A((f32, f32)),
    B(u32),
}

#[derive(Debug, PartialEq)]
struct Custom {
    foo: u64,
    items: Vec<Item>,
}

impl ToBytes for Custom {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        let mut bytes = vec![];
        // Store foo (in big endian).
        bytes.extend(&self.foo.to_bytes()?);

        // Store the number of items (in big endian).
        bytes.extend(&(self.items.len() as u16).to_bytes()?);

        // Use a bitfield to more compactly represent
        // whether an Item is an A or B.
        let mut bitfield = vec![0u8; (self.items.len() + 7) / 8];
        for (i, item) in self.items.iter().enumerate() {
            bitfield[i / 8] |= match item {
                Item::A(_) => 0,
                Item::B(_) => 1,
            } << (i % 8);
        }
        bytes.extend(bitfield);

        // Write out each item serially.
        for item in self.items.iter() {
            bytes.extend(match item {
                Item::A(x) => x.to_bytes()?,
                Item::B(x) => x.to_bytes()?,
            });
        }
        Ok(bytes)
    }
}

impl FromBytes for Custom {
    fn from_bytes(src: &[u8]) -> Result<(usize, Self), Error> {
        let mut offset = 0;

        // Read foo (in big endian).
        let (size, foo) = u64::from_bytes(&src[offset..])?;
        offset += size;

        // Read the number of items (in big endian).
        let (size, item_len) = u16::from_bytes(&src[offset..])?;
        offset += size;

        // Read the bitfield data but keep it as a u8 slice.
        let bitfield_len = ((item_len + 7) / 8) as usize;
        let bitfield = &src[offset..offset + bitfield_len];
        offset += bitfield_len;

        // Read the items, checking the bitfield to know whether an item
        // is an A or a B.
        let mut items = vec![];
        for i in 0..item_len as usize {
            if (bitfield[i / 8] >> (i % 8)) & 1 == 0 {
                let (size, x) = <(f32, f32)>::from_bytes(&src[offset..])?;
                items.push(Item::A(x));
                offset += size;
            } else {
                let (size, x) = u32::from_bytes(&src[offset..])?;
                items.push(Item::B(x));
                offset += size;
            }
        }
        Ok((offset, Custom { foo, items }))
    }
}

#[test]
fn custom() -> Result<(), Error> {
    let custom = Custom {
        foo: 1234567890123456789,
        items: vec![Item::A((3.0, 4.2)), Item::B(1337), Item::A((5.5, 6.6))],
    };
    let bytes = custom.to_bytes()?;
    assert_eq![
        bytes,
        vec![
            17, 34, 16, 244, 125, 233, 129, 21, 0, 3, 2, 64, 64, 0, 0, 64, 134, 102, 102, 0, 0, 5,
            57, 64, 176, 0, 0, 64, 211, 51, 51
        ]
    ];
    let (size, custom_deserialized) = Custom::from_bytes(&bytes)?;
    assert_eq![custom, custom_deserialized];
    assert_eq![size, bytes.len()];
    Ok(())
}
