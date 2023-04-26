use std::fmt;
use ary::ary;
use opendp::accuracy::{accuracy_to_discrete_gaussian_scale, accuracy_to_discrete_laplacian_scale};
use opendp::core::Transformation;
use opendp::domains::{AllDomain, VectorDomain};
use opendp::measurements::{make_base_discrete_gaussian, make_base_discrete_laplace};
use opendp::measures::ZeroConcentratedDivergence;
use opendp::metrics::{L2Distance, SymmetricDistance};
use opendp::transformations::{make_count_by_categories, make_select_column, make_split_dataframe};

use crate::dataset::CsvDataSet;
use crate::noiser::NoiseType::{Gaussian, Laplace};

#[derive(Clone)]
pub struct Noiser<'a> {
    dataset: &'a CsvDataSet<'a>,
    pub aggregate_field: &'a String,
    pub noise_type: NoiseType,
    pub accuracy: usize,
    pub alpha: f64,
    pub aggregated_data: Vec<u64>,
    pub noised_data: Vec<u64>,
}

#[derive(Clone, Copy, PartialEq)]
pub enum NoiseType {
    Laplace,
    Gaussian,
}

impl fmt::Display for NoiseType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Laplace => write!(f, "Laplace"),
            Gaussian => write!(f, "Gaussian"),
        }
    }
}

pub trait NoiseApplier<'a> {
    fn new(dataset: &'a CsvDataSet, aggregate_field: &'a String) -> Self;
    fn toggle_noise_type(&mut self);
    fn increase_noise(&mut self);
    fn decrease_noise(&mut self);
    fn refresh_data(&mut self);
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

const ACCURACY_VALUES: [usize; 100] = ary![=> ..100: |i| i];

impl<'a> Noiser<'a> {
    fn clear_previous_data(&mut self) {
        self.aggregated_data.clear();
        self.noised_data.clear();
    }

    fn aggregate_data(&self) -> Option<Vec<u64>> {
        let chain = aggregate_data_chain(&self, self.aggregate_field)?;
        let aggregated_data = chain.invoke(&self.dataset.data).ok()?;
        Option::from(aggregated_data)
    }

    fn noised_data(&self, aggregated_data: &Vec<u64>) -> Option<Vec<u64>> {
        match self.noise_type {
            Laplace => {
                // sensitivity / epsilon
                let scale = accuracy_to_discrete_laplacian_scale(self.accuracy as f64, self.alpha).unwrap();
                let discrete_lp = make_base_discrete_laplace::<VectorDomain<AllDomain<u64>>, _>(
                    scale
                ).ok()?;
                Option::from(discrete_lp.invoke(&aggregated_data).unwrap())
            }
            Gaussian => {
                let scale = accuracy_to_discrete_gaussian_scale(self.accuracy as f64, self.alpha).unwrap();
                let discrete_gaussian =
                    make_base_discrete_gaussian::<VectorDomain<AllDomain<u64>>, ZeroConcentratedDivergence<f64>, f64>(
                        scale
                    ).ok()?;
                Option::from(discrete_gaussian.invoke(&aggregated_data).unwrap())
            }
        }
    }
}

impl<'a> NoiseApplier<'a> for Noiser<'a> {
    fn new(dataset: &'a CsvDataSet, aggregate_field: &'a String) -> Self {
        Noiser {
            dataset,
            aggregate_field,
            noise_type: Laplace,
            accuracy: 0,
            alpha: 0.05,
            aggregated_data: Vec::<u64>::new(),
            noised_data: Vec::<u64>::new(),
        }
    }

    fn toggle_noise_type(&mut self) {
        if self.noise_type == Laplace {
            self.noise_type = Gaussian;
        } else {
            self.noise_type = Laplace;
        }
        self.refresh_data()
    }

    fn increase_noise(&mut self) {
        self.accuracy = (self.accuracy + 1) % ACCURACY_VALUES.len();
        self.refresh_data()
    }

    fn decrease_noise(&mut self) {
        self.accuracy = (self.accuracy + ACCURACY_VALUES.len() - 1) % ACCURACY_VALUES.len();
        self.refresh_data()
    }

    fn refresh_data(&mut self) {
        self.clear_previous_data();
        self.aggregated_data = self.aggregate_data().unwrap();
        self.noised_data = self.noised_data(&self.aggregated_data).unwrap();
    }
}
