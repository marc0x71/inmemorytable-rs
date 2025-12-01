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
        Ok(table) => {
            println!("tabella aperta: {table:?}");
            println!("  count = {}", table.count());
            println!("  capacity = {}", table.capacity());

            for i in 0..table.count() as i32 {
                let record = table.find(i);
                println!("\t#{i} - {:?}", record);
            }

            dbg!(&table.find(4));
            dbg!(&table.find(42));
        }
        Err(e) => eprintln!("errore: {e}"),
    }
}
