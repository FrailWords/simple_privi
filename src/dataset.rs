const COLUMNS: &'static [&'static str] = &["age", "sex", "educ", "race", "income", "married"];

pub struct CsvDataSet<'a> {
    pub data: &'a String,
}

impl<'a> CsvDataSet<'a> {
    pub fn columns(&self) -> Vec<&'static str> {
        Vec::from(COLUMNS)
    }

    pub fn aggregate_buckets(&self, field: &String) -> Vec<String> {
        match field.as_str() {
            "income" => (10000u32..210000).step_by(10000).map(|x| x.to_string()).collect::<Vec<_>>(),
            &_ => (1u8..21).map(|x| x.to_string()).collect::<Vec<_>>(),
        }
    }
}