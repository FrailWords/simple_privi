const COLUMNS: &'static [&'static str] = &["age", "sex", "educ", "race", "income", "married"];

pub struct CsvDataSet {
    pub data: String,
}

impl CsvDataSet {
    pub fn columns(&self) -> Vec<&'static str> {
        Vec::from(COLUMNS)
    }

    pub fn aggregate_buckets(&self, field: &String) -> Vec<String> {
        (1u8..21).map(|x| x.to_string()).collect::<Vec<_>>()
    }
}