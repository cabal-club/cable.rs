use desert::{CountBytes, Error, FromBytes, ToBytes};

#[test]
fn builtins() -> Result<(), Error> {
    assert_eq![u8::from_bytes(&[5])?, (1, 5)];
    assert_eq![u16::from_bytes(&[13, 12])?, (2, 3340)];
    assert_eq![u32::from_bytes(&[5, 6, 7, 8])?, (4, 84281096)];
    assert_eq![5u8.to_bytes()?, vec!(5)];
    assert_eq![3340u16.to_bytes()?, vec!(13, 12)];
    assert_eq![84281096u32.to_bytes()?, vec!(5, 6, 7, 8)];
    assert_eq![u8::count_from_bytes(&[5])?, 1];
    assert_eq![u16::count_from_bytes(&[13, 12])?, 2];
    assert_eq![u32::count_from_bytes(&[5, 6, 7, 8])?, 4];
    assert_eq![vec![7u8, 8u8, 9u8].to_bytes()?, vec!(3, 7, 8, 9)];
    assert_eq![
        vec![7u32, 8u32, 9u32].to_bytes()?,
        vec!(12, 0, 0, 0, 7, 0, 0, 0, 8, 0, 0, 0, 9)
    ];
    assert_eq![<Vec<u8>>::count_from_bytes(&[3, 7, 8, 9])?, 4];
    assert_eq![<Vec<u8>>::from_bytes(&[3, 7, 8, 9])?, (4, vec!(7, 8, 9))];
    assert_eq![
        <Vec<u32>>::from_bytes(&[12, 0, 0, 0, 7, 0, 0, 0, 8, 0, 0, 0, 9])?,
        (13, vec!(7u32, 8u32, 9u32))
    ];
    assert_eq![vec!(7u32, 8u32, 9u32).count_bytes(), 13];
    {
        let v: Vec<u16> = (0..500).map(|i| i % 5 + (i % 3) * 500).collect();
        let bytes = v.to_bytes()?;
        let mut with_extra: Vec<u8> = bytes.clone();
        let extra: Vec<u8> = (0..100)
            .map(|i| ((i % 6 + (i % 9) * 300) % 256) as u8)
            .collect();
        with_extra.extend(extra);
        assert_eq![bytes.len(), 1002];
        assert_eq![<Vec<u16>>::from_bytes(&bytes)?, (bytes.len(), v.clone())];
        assert_eq![<Vec<u16>>::count_from_bytes(&bytes)?, bytes.len()];
        assert_eq![
            <Vec<u16>>::from_bytes(&with_extra[..])?,
            (bytes.len(), v.clone())
        ];
        assert_eq![<Vec<u16>>::count_from_bytes(&with_extra[..])?, bytes.len()];
    }
    Ok(())
}

#[test]
fn booleans() -> Result<(), Error> {
    assert_eq![true.count_bytes(), 1];
    assert_eq![vec!(true, false).count_bytes(), 3];
    Ok(())
}

#[test]
fn tuples() -> Result<(), Error> {
    assert_eq![(3u8, 4u8).count_bytes(), 2];
    assert_eq![(6u16, 7u16, 8u16).count_bytes(), 6];
    assert_eq![(3u8, 4u8).to_bytes()?, vec!(3, 4)];
    assert_eq![(6u16, 7u16, 8u16).to_bytes()?, vec!(0, 6, 0, 7, 0, 8)];
    assert_eq![
        <(u16, u16, u16)>::count_from_bytes(&[0, 6, 0, 7, 0, 8, 200, 201, 202, 203, 204])?,
        6
    ];
    assert_eq![(6u16, 7u8, 8u16).to_bytes()?, vec!(0, 6, 7, 0, 8)];
    assert_eq![
        <(u16, u8, u16)>::count_from_bytes(&[0, 6, 7, 0, 8, 200, 201, 202, 203, 204])?,
        5
    ];
    Ok(())
}

#[test]
fn arrays() -> Result<(), Error> {
    assert_eq![[3u8, 4u8].count_bytes(), 2];
    assert_eq![[6u16, 7u16, 8u16].count_bytes(), 6];
    assert_eq![[3u8, 4u8].to_bytes()?, vec!(3, 4)];
    assert_eq![[6u16, 7u16, 8u16].to_bytes()?, vec!(0, 6, 0, 7, 0, 8)];
    assert_eq![
        <[u16]>::count_from_bytes(&[6, 0, 6, 0, 7, 0, 8, 200, 201, 202, 203, 204])?,
        7
    ];
    assert_eq![
        <[u16; 3]>::count_from_bytes(&[0, 6, 0, 7, 0, 8, 200, 201, 202, 203, 204])?,
        6
    ];
    Ok(())
}

#[test]
fn vectors() -> Result<(), Error> {
    {
        let xs: Vec<u8> = vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13];
        assert_eq![
            xs.to_bytes()?,
            vec![14, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13]
        ];
        assert_eq![
            <Vec<u8>>::from_bytes(&xs.to_bytes()?)?,
            (15, vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13])
        ];
    }
    {
        let xs: Vec<u8> = (0..200)
            .map(|x| (((x * 137 + 24) as u16) % 256) as u8)
            .collect();
        assert_eq![<Vec<u8>>::from_bytes(&xs.to_bytes()?)?, (xs.len() + 2, xs)];
    }
    Ok(())
}

#[test]
fn nested() -> Result<(), Error> {
    {
        let xs: Vec<u8> = (0..200)
            .map(|x| (((x * 137 + 24) as u16) % 256) as u8)
            .collect();
        let len = xs.len();
        let copy = xs.clone();
        assert_eq![
            <(u8, u32, u16, Vec<u8>)>::from_bytes(&(4u8, 5u32, 6u16, xs).to_bytes()?)?,
            (len + 2 + 7, (4u8, 5u32, 6u16, copy))
        ];
    }
    {
        let xs = vec![
            211, 245, 162, 214, 25, 238, 66, 38, 34, 37, 155, 210, 230, 76, 17, 182, 128, 200, 112,
            118, 77, 62, 24, 32, 201, 100, 198, 21, 244, 182, 52, 81, 158, 55, 43, 0, 61, 115, 92,
            4, 6, 80, 197, 51, 241, 19, 131, 140, 238, 147, 19, 43, 149, 21, 93, 56, 190, 249, 106,
            15, 20, 166, 69, 201, 121, 36, 111, 180,
        ];
        assert_eq![<Vec<u8>>::from_bytes(&xs.to_bytes()?)?, (69, xs.clone())];
        type T = (((f32, f32), (f32, f32), f32), Vec<u8>);
        let obj: T = (((-0.79, -0.68), (0.46, 0.46), 40.73), xs.clone());
        assert_eq![T::from_bytes(&obj.to_bytes()?)?, (89, obj)];
    }
    Ok(())
}
