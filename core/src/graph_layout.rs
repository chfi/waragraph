use arrow2::{
    array::{BinaryArray, Float32Array, StructArray, UInt32Array, Utf8Array},
    chunk::Chunk,
    datatypes::{DataType, Field, Metadata, Schema},
    offset::Offsets,
};

pub struct GraphLayout {
    pub xs: Float32Array,
    pub ys: Float32Array,

    pub aabb_min: [f32; 2],
    pub aabb_max: [f32; 2],
}

impl GraphLayout {
    pub fn arrow_schema(&self) -> Schema {
        let mut metadata: Metadata = [
            ("aabb_min_x", self.aabb_min[0]),
            ("aabb_min_y", self.aabb_min[1]),
            ("aabb_max_x", self.aabb_max[0]),
            ("aabb_max_y", self.aabb_max[1]),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();

        Schema::from(vec![
            Field::new("x", DataType::Float32, false),
            Field::new("y", DataType::Float32, false),
        ])
        .with_metadata(metadata)
    }

    // pub fn to_arrow_ipc(

    // pub fn from_chunk(
    //     )

    #[cfg(not(target = "wasm32-unknown-unknown"))]
    pub fn from_tsv(
        tsv_path: impl AsRef<std::path::Path>,
    ) -> std::io::Result<Self> {
        use std::io::prelude::*;
        use std::io::Cursor;
        // use ultraviolet::Vec2;

        // let tsv_text = tsv_text
        //     .as_string()
        //     .ok_or_else(|| format!("TSV could not be read as text"))?;

        // let cursor = Cursor::new(tsv_text.as_bytes());

        let reader =
            std::fs::File::open(tsv_path).map(std::io::BufReader::new)?;

        let mut xs = Vec::new();
        let mut ys = Vec::new();

        let mut min_x = std::f32::MAX;
        let mut min_y = std::f32::MAX;

        let mut max_x = std::f32::MIN;
        let mut max_y = std::f32::MIN;

        for (i, line) in reader.lines().enumerate() {
            if i == 0 {
                continue;
            }

            let Ok(line) = line else { continue };
            let line = line.trim();

            let mut fields = line.split_ascii_whitespace();

            let _id = fields.next();

            let x = fields.next().unwrap().parse::<f32>().unwrap();
            let y = fields.next().unwrap().parse::<f32>().unwrap();
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);

            xs.push(x);
            ys.push(y);
        }

        Ok(Self {
            xs: Float32Array::from_vec(xs),
            ys: Float32Array::from_vec(ys),
            aabb_min: [min_x, min_y],
            aabb_max: [max_x, max_y],
        })
    }
}
