use std::env::args;
use std::fs::File;
use std::io::Read;

use pyrite::parse_segment;
use pyrite::try_take_frame;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = args();
    args.next();

    let mut data_vec = vec![];
    let mut file = File::open(args.next().unwrap())?;
    file.read_to_end(&mut data_vec)?;

    let mut data_bytes = data_vec.as_ref();
    let mut segments = vec![];

    loop {
        let (leftover, segment) = parse_segment(data_bytes).unwrap();
        segments.push(segment);

        if leftover.is_empty() {
            break;
        }

        data_bytes = leftover;
    }

    println!("{:?}", segments.first());

    while let Some(frame) = try_take_frame(&mut segments) {
        frame
            .get_pixels()
            .unwrap()
            .save_with_format(args.next().unwrap(), image::ImageFormat::Png);
        break;
    }

    println!("{:?}", segments.len());

    Ok(())
}
