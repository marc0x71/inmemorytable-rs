use inmemorytable::{record::TableRecord, table::Table};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Record {
    number: i32,
    value: f64,
}
impl TableRecord for Record {
    type Key = i32;

    fn key(&self) -> Self::Key {
        todo!()
    }

    fn indexes() -> Vec<inmemorytable::index::IndexDef<Self>> {
        vec![]
    }
}

fn main() {
    println!("Reader!");

    let t = Table::<Record>::open("table1");
    match t {
        Ok(table) => match table.destroy() {
            Ok(_) => {
                println!("tabella <table1> distrutta")
            }
            Err(e) => eprintln!("errore: {e}"),
        },
        Err(e) => eprintln!("errore: {e}"),
    }
}
