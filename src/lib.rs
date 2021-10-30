pub mod utils;
pub mod wasm_iface;

use nom::bytes::streaming::*;
use nom::combinator::*;
use nom::number::streaming::*;
use nom::IResult;

use simple_matrix::Matrix;

use image::Rgba;
use image::RgbaImage;

#[derive(Debug)]
pub enum SegmentType {
    PDS = 0x14,
    ODS = 0x15,
    PCS = 0x16,
    WDS = 0x17,
    END = 0x80,
}

#[derive(Debug)]
pub struct Header {
    pts: u32,
    dts: u32,
    segment_type: SegmentType,
    segment_size: u16,
}

pub fn parse_header(data: &[u8]) -> IResult<&[u8], Header> {
    // magic number (0x5047) PG
    let (data, _) = tag("PG")(data)?;

    let (data, pts) = be_u32(data)?;
    let (data, dts) = be_u32(data)?;
    let (data, segment_type) = be_u8(data)?;
    let segment_type = match segment_type {
        0x14 => SegmentType::PDS,
        0x15 => SegmentType::ODS,
        0x16 => SegmentType::PCS,
        0x17 => SegmentType::WDS,
        0x80 => SegmentType::END,
        _ => todo!(),
    };
    let (data, segment_size) = be_u16(data)?;

    Ok((
        data,
        Header {
            pts,
            dts,
            segment_type,
            segment_size,
        },
    ))
}

#[derive(Debug)]
pub struct PCSegment {
    header: Header,
    width: u16,
    height: u16,
    fps: u8,
    composition_n: u16,
    composition_s: u8,
    pallete_update: bool,
    pallete_id: u8,
    num_objects: u8,
    objects: Vec<CompObj>,
}

pub fn parse_pcs(data: &[u8], header: Header) -> IResult<&[u8], PCSegment> {
    let (data, width) = be_u16(data)?;
    let (data, height) = be_u16(data)?;
    let (data, fps) = be_u8(data)?;
    let (data, composition_n) = be_u16(data)?;
    let (data, composition_s) = be_u8(data)?;
    let (data, pallete_update) = be_u8(data)?;
    let pallete_update = pallete_update == 0x80;

    let (data, pallete_id) = be_u8(data)?;
    let (mut data, num_objects) = be_u8(data)?;

    let mut objects = vec![];

    for _ in 0..num_objects {
        let (leftover, object) = parse_compobj(data)?;
        objects.push(object);

        data = leftover;
    }

    Ok((
        data,
        PCSegment {
            header,
            width,
            height,
            fps,
            composition_n,
            composition_s,
            pallete_update,
            pallete_id,
            num_objects,
            objects,
        },
    ))
}

#[derive(Debug)]
pub struct CompObj {
    id: u16,
    wid: u8,
    cropped_flag: bool,
    x: u16,
    y: u16,
    crop_x: Option<u16>,
    crop_y: Option<u16>,
    crop_width: Option<u16>,
    crop_height: Option<u16>,
}

pub fn parse_compobj(data: &[u8]) -> IResult<&[u8], CompObj> {
    let (data, id) = be_u16(data)?;
    let (data, wid) = be_u8(data)?;
    let (data, cropped_flag) = be_u8(data)?;
    let cropped_flag = cropped_flag == 0x40;

    let (data, x) = be_u16(data)?;
    let (data, y) = be_u16(data)?;

    if cropped_flag {
        let (data, crop_x) = be_u16(data)?;
        let (data, crop_y) = be_u16(data)?;
        let (data, crop_width) = be_u16(data)?;
        let (data, crop_height) = be_u16(data)?;

        Ok((
            data,
            CompObj {
                id,
                wid,
                cropped_flag,
                x,
                y,
                crop_x: Some(crop_x),
                crop_y: Some(crop_y),
                crop_width: Some(crop_width),
                crop_height: Some(crop_height),
            },
        ))
    } else {
        Ok((
            data,
            CompObj {
                id,
                wid,
                cropped_flag,
                x,
                y,
                crop_x: None,
                crop_y: None,
                crop_width: None,
                crop_height: None,
            },
        ))
    }
}

#[derive(Debug)]
pub struct WDSegment {
    header: Header,
    n_windows: u8,
    wid: u8,
    x_pos: u16,
    y_pos: u16,
    width: u16,
    height: u16,
}

pub fn parse_wdseg(data: &[u8], header: Header) -> IResult<&[u8], WDSegment> {
    let (data, n_windows) = be_u8(data)?;
    let (data, wid) = be_u8(data)?;
    let (data, x_pos) = be_u16(data)?;
    let (data, y_pos) = be_u16(data)?;
    let (data, width) = be_u16(data)?;
    let (data, height) = be_u16(data)?;

    Ok((
        data,
        WDSegment {
            header,
            n_windows,
            wid,
            x_pos,
            y_pos,
            width,
            height,
        },
    ))
}

#[derive(Debug)]
pub struct PDSegment {
    header: Header,
    pid: u8,
    version: u8,
    palletes: Vec<PalleteEntry>,
}

#[derive(Debug)]
pub struct PalleteEntry {
    eid: u8,
    y: u8,
    cr: u8,
    cb: u8,
    alpha: u8,
}

impl PalleteEntry {
    pub fn to_rgba(&self) -> [u8; 4] {
        let conv_matrix = Matrix::from_iter(
            3,
            3,
            vec![1.0f64, 1., 1., 0., -0.1873, 1.8556, 1.5748, -0.4682, 0.],
        );

        let ycrcb = Matrix::from_iter(
            3,
            1,
            vec![
                self.y as f64 / 255.0,
                self.cb as f64 / 255.0,
                self.cr as f64 / 255.0,
            ],
        );

        let rgb = conv_matrix * ycrcb;

        [
            (*rgb.get(0, 0).unwrap() * 255.0) as u8,
            (*rgb.get(1, 0).unwrap() * 255.0) as u8,
            (*rgb.get(2, 0).unwrap() * 255.0) as u8,
            self.alpha,
        ]
    }
}

pub fn parse_pallete(data: &[u8]) -> IResult<&[u8], PalleteEntry> {
    let (data, eid) = be_u8(data)?;
    let (data, y) = be_u8(data)?;
    let (data, cr) = be_u8(data)?;
    let (data, cb) = be_u8(data)?;
    let (data, alpha) = be_u8(data)?;

    Ok((
        data,
        PalleteEntry {
            eid,
            y,
            cr,
            cb,
            alpha,
        },
    ))
}

pub fn parse_pds(data: &[u8], header: Header) -> IResult<&[u8], PDSegment> {
    let (data, pid) = be_u8(data)?;
    let (data, version) = be_u8(data)?;

    // takes all bytes of this segment -2 which are used above.
    let (data, pallete_bytes) = take(header.segment_size - 2)(data)?;

    let mut palletes_iter = iterator(pallete_bytes, parse_pallete);
    let palletes = palletes_iter.collect::<Vec<_>>();
    let _ = palletes_iter.finish();

    Ok((
        data,
        PDSegment {
            header,
            pid,
            version,
            palletes,
        },
    ))
}

pub struct ODSegment {
    header: Header,
    id: u16,
    version: u8,
    seq_flag: u8,
    data_len: u32, // is actually 24-bits long
    width: u16,
    height: u16,
    object_data: Vec<u8>,
}

impl std::fmt::Debug for ODSegment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ODSegment")
            .field("header", &self.header)
            .field("id", &self.id)
            .field("version", &self.version)
            .field("seq_flag", &self.seq_flag)
            .field("data_len", &self.data_len)
            .field("width", &self.width)
            .field("height", &self.height)
            .finish()
    }
}

pub fn parse_ods(data: &[u8], header: Header) -> IResult<&[u8], ODSegment> {
    let (data, id) = be_u16(data)?;
    let (data, version) = be_u8(data)?;
    let (data, seq_flag) = be_u8(data)?;
    let (data, data_len) = be_u24(data)?;
    let (data, width) = be_u16(data)?;
    let (data, height) = be_u16(data)?;
    let (data, object_data) = take(data_len - 4)(data)?;

    Ok((
        data,
        ODSegment {
            header,
            id,
            version,
            seq_flag,
            data_len,
            width,
            height,
            object_data: object_data.to_vec(),
        },
    ))
}

#[derive(Debug)]
pub struct ENDSegment {
    header: Header,
}

#[derive(Debug)]
pub enum Segment {
    PCS(PCSegment),
    WDS(WDSegment),
    PDS(PDSegment),
    ODS(ODSegment),
    END(ENDSegment),
}

pub fn parse_segment(data: &[u8]) -> IResult<&[u8], Segment> {
    let (data, header) = parse_header(data)?;
    let (data, segment) = match header.segment_type {
        SegmentType::PCS => {
            let (data, seg) = parse_pcs(data, header)?;
            (data, Segment::PCS(seg))
        }
        SegmentType::WDS => {
            let (data, seg) = parse_wdseg(data, header)?;
            (data, Segment::WDS(seg))
        }
        SegmentType::PDS => {
            let (data, seg) = parse_pds(data, header)?;
            (data, Segment::PDS(seg))
        }
        SegmentType::ODS => {
            let (data, seg) = parse_ods(data, header)?;
            (data, Segment::ODS(seg))
        }
        SegmentType::END => (data, Segment::END(ENDSegment { header })),
    };

    Ok((data, segment))
}

#[derive(Debug)]
pub struct Frame {
    pcs: PCSegment,
    wds: WDSegment,
    pds: Option<PDSegment>,
    ods: Option<ODSegment>,
    end: ENDSegment,
}

const BLACK: [u8; 4] = [0, 0, 0, 0];

impl Frame {
    pub fn pts(&self) -> u32 {
        self.pcs.header.pts
    }

    pub fn get_pixels(&self) -> Option<RgbaImage> {
        let object_data = self.ods.as_ref()?.object_data.as_slice();
        let (_, decoded_pixels) = decode_rle(object_data).unwrap();

        let ods = self.ods.as_ref()?;
        let (w, h) = (ods.width, ods.height);
        let mut image = RgbaImage::new(w as u32, h as u32);

        for (i, color_id) in decoded_pixels.iter().enumerate() {
            let x = (i % w as usize) as u16;
            let y = (i / w as usize) as u16;

            let color: Rgba<u8> = if *color_id == 0 {
                BLACK.into()
            } else {
                let colors = &self.pds.as_ref()?.palletes;
                let color = colors
                    .iter()
                    .find(|x| x.eid == *color_id)
                    .map(|ycrcb| ycrcb.to_rgba())
                    .unwrap_or(BLACK);

                color.into()
            };

            image.put_pixel(x as u32, y as u32, color);
        }

        Some(image)
    }

    // FIXME: The code here might be incorrect if we have multiple objects/subpics.
    pub fn image_x(&self) -> Option<u16> {
        Some(self.pcs.objects.first()?.x)
    }

    pub fn image_y(&self) -> Option<u16> {
        Some(self.pcs.objects.first()?.y)
    }
}

fn is_color(byte: u8) -> bool {
    byte >> 7 == 1
}

fn is_long(byte: u8) -> bool {
    (byte >> 6) & 0b1 == 1
}

pub fn decode_rle(mut data: &[u8]) -> IResult<&[u8], Vec<u8>> {
    let mut output = vec![];

    while !data.is_empty() {
        // check first byte color
        let (leftover, color) = be_u8(data)?;
        data = leftover;
        match color {
            0x00 => {}
            _ => {
                output.push(1);
                continue;
            }
        };

        // check second byte for length
        let (leftover, length) = be_u8(data)?;
        data = leftover;
        let info = match length {
            0x00 => {
                continue;
            }
            x => x,
        };

        let is_color = is_color(info);
        let big_len = is_long(info);

        let len_u8 = info & 0b0011_1111;
        assert_eq!(len_u8 >> 6, 0);

        let len = if big_len {
            let (leftover, len2_u8) = be_u8(data)?;
            let buf = [len_u8, len2_u8];
            data = leftover;
            u16::from_be_bytes(buf)
        } else {
            len_u8 as u16
        };

        let (leftover, color) = if is_color {
            be_u8(data)?
        } else {
            // use preferred color
            (data, 0)
        };

        for _x in 0..len {
            output.push(color);
        }

        data = leftover;
    }

    Ok((data, output))
}

pub fn try_take_frame(segments: &mut Vec<Segment>) -> Option<Frame> {
    // if theres no `END` segment it means we can only create a partial frame.
    if segments
        .iter()
        .find(|x| matches!(x, Segment::END(_)))
        .is_none()
    {
        return None;
    }

    let pcs = match take_if!(segments, |x| matches!(x, Segment::PCS(_))) {
        Some(Segment::PCS(x)) => x,
        _ => unreachable!(),
    };

    let wds = match take_if!(segments, |x| matches!(x, Segment::WDS(_))) {
        Some(Segment::WDS(x)) => x,
        _ => unreachable!(),
    };

    let pds = match take_if!(segments, |x| matches!(x, Segment::PDS(_))) {
        Some(Segment::PDS(x)) => Some(x),
        None => None,
        _ => unreachable!(),
    };

    let ods = match take_if!(segments, |x| matches!(x, Segment::ODS(_))) {
        Some(Segment::ODS(x)) => Some(x),
        None => None,
        _ => unreachable!(),
    };

    let end = match take_if!(segments, |x| matches!(x, Segment::END(_))) {
        Some(Segment::END(x)) => x,
        _ => unreachable!(),
    };

    Some(Frame {
        pcs,
        wds,
        pds,
        ods,
        end,
    })
}
