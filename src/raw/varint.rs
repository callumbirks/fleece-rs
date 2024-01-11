const MAX_LEN: usize = 10;

pub fn read(data: &[u8]) -> (usize, u64) {
    if data.len() < 2 {
        return (0, 0);
    }
    if data.len() == 2 {
        return (1, data[1] as u64);
    }

    let mut shift = 0;
    let mut res = 0_u64;
    let end: usize = data.len().min(MAX_LEN + 1);
    for (i, byte) in data[1..end].iter().enumerate() {
        if *byte >= 0x80 {
            res |= ((*byte & 0x7F) as u64) << shift;
            shift += 7;
        } else {
            res |= (*byte as u64) << shift;
            // Make sure the varint is below the max length
            if i == MAX_LEN && *byte > 1 {
                return (0, 0);
            }
            return (i + 1, res);
        }
    }
    (0, 0)
}

pub fn write(out: &mut [u8], value: u64) -> usize {
    let mut value = value;
    let mut bytes_written: usize = 0;
    while value >= 0x80 {
        out[bytes_written] = (value & 0xFF) as u8 | 0x80;
        value >>= 7;
        bytes_written += 1;
    }
    out[bytes_written] = value as u8;
    bytes_written + 1
}
