use opendp::accuracy::accuracy_to_discrete_laplacian_scale;
use opendp::core::Transformation;
use opendp::domains::{AllDomain, VectorDomain};
use opendp::measurements::make_base_discrete_laplace;
use opendp::metrics::{L2Distance, SymmetricDistance};
use opendp::transformations::{make_count_by_categories, make_select_column, make_split_dataframe};

use crate::dataset::{CsvDataSet};

#[derive(Clone, Copy)]
pub struct Noiser<'a> {
    dataset: &'a CsvDataSet,
}

pub trait NoiseApplier<'a> {
    fn new(dataset: &'a CsvDataSet) -> Self;
    fn aggregate_data(&self, aggregate_field: &String) -> Option<Vec<u64>>;
    fn noised_data(&self, aggregated_data: &Vec<u64>, accuracy: i64, alpha: f64) -> Option<Vec<u64>>;
}

const CSV_SEPARATOR: &'static str = ",";

fn aggregate_data_chain(noiser: &Noiser, aggregate_field: &String) -> Option<Transformation<AllDomain<String>, VectorDomain<AllDomain<u64>>, SymmetricDistance, L2Distance<u8>>> {
    let aggregate_buckets = noiser.dataset.aggregate_buckets(aggregate_field);
    let column_names = noiser.dataset.columns().iter().map(|s| s.to_string()).collect();

    // transformers chain
    let df_transformer = make_split_dataframe(Option::from(CSV_SEPARATOR), column_names).ok()?;
    let aggregate_column = make_select_column::<String, String>(aggregate_field.clone()).ok()?;
    let count_by_aggr_column = make_count_by_categories::<L2Distance<u8>, String, u64>(aggregate_buckets, true).ok()?;
    let chain = (df_transformer >> aggregate_column >> count_by_aggr_column).ok()?;
    Option::from(chain)
}


impl<'a> NoiseApplier<'a> for Noiser<'a> {
    fn new(dataset: &'a CsvDataSet) -> Self {
        Noiser {
            dataset
        }
    }

    fn aggregate_data(&self, aggregate_field: &String) -> Option<Vec<u64>> {
        let chain = aggregate_data_chain(&self, aggregate_field)?;
        let aggregated_data = chain.invoke(&self.dataset.data).ok()?;
        Option::from(aggregated_data)
    }

    fn noised_data(&self, aggregated_data: &Vec<u64>, accuracy: i64, alpha: f64) -> Option<Vec<u64>> {
        //* `scale` - Noise scale parameter for the laplace distribution. `scale` == sqrt(2) * standard_deviation.
        let scale = accuracy_to_discrete_laplacian_scale(accuracy as f64, alpha).unwrap();
        let discrete_lp = make_base_discrete_laplace::<VectorDomain<AllDomain<u64>>, _>(
            scale
        ).ok()?;
        Option::from(discrete_lp.invoke(&aggregated_data).unwrap())
    }
}
