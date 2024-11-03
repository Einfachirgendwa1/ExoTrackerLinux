mod data_provider;
mod model;
mod utils;
mod view;

use crate::model::create_model;
use utils::*;

fn main() {
    nannou::app(create_model).run();
}
