use std::{error::Error, io, process};
use std::fs::File;

#[derive(Debug, serde::Deserialize, Copy, Clone)]
pub struct Record {
    pub age: u8,
    pub sex: u8,
    pub educ: u8,
    pub race: u8,
    pub income: u64,
    pub married: u8
}

pub fn read_data() -> Result<Vec<Record>, Box<dyn Error>> {
    let file = File::open("data/data.csv")?;
    let mut rdr = csv::Reader::from_reader(file);
    let mut records = Vec::<Record>::new();
    for result in rdr.deserialize() {
        // Notice that we need to provide a type hint for automatic
        // deserialization.
        let record: Record = result?;
        records.push(record);
    }
    Ok(records)
}
