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
        self.number
    }

    fn indexes() -> Vec<inmemorytable::index::IndexDef<Self>> {
        vec![]
    }
}

fn main() {
    println!("Writer!");
    let t = Table::<Record>::create("table1", 100);
    match t {
        Ok(mut table) => {
            println!("tabella creata: {table:?}");
            println!("  count = {}", table.count());
            println!("  capacity = {}", table.capacity());

            for i in 0..10 {
                let record = Record {
                    number: i,
                    value: 100.0 / (i as f64),
                };
                let _ = table.insert(&record);
            }

            println!("  count = {}", table.count());
            println!("  capacity = {}", table.capacity());
        }
        Err(e) => eprintln!("errore: {e}"),
    }
}
