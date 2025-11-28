use inmemorytable::{record::TableRecord, table::Table};
use rand::{Rng, distr::Alphanumeric};
use serde::{Deserialize, Serialize};

mod common;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
struct TestData {
    number: i32,
    value: f64,
}

impl TableRecord for TestData {
    type Key = i32;

    fn key(&self) -> Self::Key {
        self.number
    }
}

fn random_table_name() -> String {
    let random_part: String = rand::rng()
        .sample_iter(&Alphanumeric)
        .take(8)
        .map(char::from)
        .collect();
    format!("table_{}", random_part)
}

#[test]
fn test_create_basic() {
    let name = random_table_name();

    let mut table = Table::<TestData>::create(&name, 10).unwrap();

    for i in 0..5 {
        let record = TestData {
            number: i,
            value: 100.0 / (i as f64),
        };
        table.insert(&record).expect("unable to insert record");
    }
    assert_eq!(table.count(), 5);
    assert_eq!(table.capacity(), 10);

    for i in 0..5 {
        let expected = TestData {
            number: i,
            value: 100.0 / (i as f64),
        };
        let got = table.find(i).unwrap().unwrap();
        assert_eq!(got, expected);
    }

    let got = table.find(44).unwrap();
    assert!(got.is_none());

    table.destroy().unwrap();

    assert!(!common::shm_exists(&name));
    assert!(!common::sem_exists(&name));
}

#[test]
fn test_create_open_basic() {
    let name = random_table_name();

    // create
    {
        let mut table = Table::<TestData>::create(&name, 10).unwrap();
        for i in 0..5 {
            let record = TestData {
                number: i,
                value: 100.0 / (i as f64),
            };
            table.insert(&record).expect("unable to insert record");
        }

        assert_eq!(table.count(), 5);
        assert_eq!(table.capacity(), 10);
    }

    // open
    {
        let table = Table::<TestData>::open(&name).unwrap();

        let expected = TestData {
            number: 4,
            value: 100.0 / 4.0,
        };
        let got = table.find(4).unwrap().unwrap();
        assert_eq!(got, expected);

        let got = table.find(44).unwrap();
        assert!(got.is_none());

        table.destroy().unwrap();

        assert!(!common::shm_exists(&name));
        assert!(!common::sem_exists(&name));
    }
}

#[test]
fn test_remove() {
    let name = random_table_name();

    let mut table = Table::<TestData>::create(&name, 10).unwrap();

    for i in 0..5 {
        let record = TestData {
            number: i,
            value: 100.0 / (i as f64),
        };
        table.insert(&record).expect("unable to insert record");
    }
    assert_eq!(table.count(), 5);
    assert_eq!(table.capacity(), 10);

    let mut exp_count = 5;
    for i in 0..5 {
        assert_eq!(table.count(), exp_count);

        table.remove(i).expect("unable to remove key");
        assert!(table.find(i).unwrap().is_none());

        exp_count -= 1;
        assert_eq!(table.count(), exp_count);
        assert_eq!(table.capacity(), 10);
    }
    assert_eq!(table.count(), 0);
    assert_eq!(table.capacity(), 10);

    table.destroy().unwrap();

    assert!(!common::shm_exists(&name));
    assert!(!common::sem_exists(&name));
}

#[test]
fn test_insert_all() {
    let name = random_table_name();

    let mut table = Table::<TestData>::create(&name, 5).unwrap();

    for i in 0..5 {
        let record = TestData {
            number: i,
            value: 100.0 / (i as f64),
        };
        table.insert(&record).expect("unable to insert record");
    }
    assert_eq!(table.count(), 5);
    assert_eq!(table.capacity(), 5);

    for i in 0..5 {
        let expected = TestData {
            number: i,
            value: 100.0 / (i as f64),
        };
        let got = table.find(i).unwrap().unwrap();
        assert_eq!(got, expected);
    }

    let mut exp_count = 5;
    for i in 0..5 {
        assert_eq!(table.count(), exp_count);

        table.remove(i).expect("unable to remove key");
        assert!(table.find(i).unwrap().is_none());

        exp_count -= 1;
        assert_eq!(table.count(), exp_count);
        assert_eq!(table.capacity(), 5);
    }
    assert_eq!(table.count(), 0);
    assert_eq!(table.capacity(), 5);

    table.destroy().unwrap();

    assert!(!common::shm_exists(&name));
    assert!(!common::sem_exists(&name));
}

#[test]
fn test_with_lock() {
    let name = random_table_name();

    // create
    let mut table = Table::<TestData>::create(&name, 10).unwrap();
    for i in 0..5 {
        let record = TestData {
            number: i,
            value: 100.0 / (i as f64),
        };
        table.insert(&record).expect("unable to insert record");
    }

    for i in 0..5 {
        table
            .update_with_lock(i, |record| {
                record.value += 100.0;
            })
            .expect("Error in with_lock");
    }
    for i in 0..5 {
        let expected = TestData {
            number: i,
            value: 100.0 + 100.0 / (i as f64),
        };
        let got = table.find(i).unwrap().unwrap();
        assert_eq!(got, expected);
    }
    assert_eq!(table.count(), 5);
    assert_eq!(table.capacity(), 10);

    table.destroy().unwrap();

    assert!(!common::shm_exists(&name));
    assert!(!common::sem_exists(&name));
}

#[test]
fn test_insert_duplicated() {
    let name = random_table_name();

    // create
    let mut table = Table::<TestData>::create(&name, 10).unwrap();
    let record = TestData {
        number: 1,
        value: 100.0 / (1 as f64),
    };
    table.insert(&record).expect("unable to insert record");
    table.insert(&record).expect_err("RecordDuplicated");

    assert_eq!(table.count(), 1);
    assert_eq!(table.capacity(), 10);

    table.destroy().unwrap();

    assert!(!common::shm_exists(&name));
    assert!(!common::sem_exists(&name));
}

#[test]
fn test_concurrent_updates_with_threads() {
    let name = random_table_name();

    // Create table with initial records (all starting at 0)
    {
        let mut table = Table::<TestData>::create(&name, 10).unwrap();
        for i in 0..5 {
            let record = TestData {
                number: i,
                value: 0.0,
            };
            table.insert(&record).unwrap();
        }
        assert_eq!(table.count(), 5);
    }

    // Spawn 5 threads, each incrementing all records 100 times
    let handles: Vec<_> = (0..5)
        .map(|thread_id| {
            let table_name = name.clone();
            std::thread::spawn(move || {
                // Each thread opens its own connection to the table
                let mut table =
                    Table::<TestData>::open(&table_name).expect("Thread failed to open table");

                for iteration in 0..100 {
                    for key in 0..5 {
                        table
                            .update_with_lock(key, |record| {
                                record.value += 1.0;
                            })
                            .unwrap_or_else(|_| {
                                panic!(
                                    "Thread {} failed at iteration {} for key {}",
                                    thread_id, iteration, key
                                )
                            });
                    }
                }
            })
        })
        .collect();

    // Wait for all threads to complete
    for (i, handle) in handles.into_iter().enumerate() {
        handle
            .join()
            .unwrap_or_else(|_| panic!("Thread {} panicked", i));
    }

    // Verify results: each record should have been incremented 500 times
    {
        let table = Table::<TestData>::open(&name).unwrap();

        for key in 0..5 {
            let record = table
                .find(key)
                .unwrap()
                .unwrap_or_else(|| panic!("Record with key {} not found", key));

            assert_eq!(
                record.value, 500.0,
                "Record {} should be 500.0 but was {}. This indicates a race condition!",
                key, record.value
            );
        }

        assert_eq!(table.count(), 5);
        table.destroy().unwrap();
    }

    assert!(!common::shm_exists(&name));
    assert!(!common::sem_exists(&name));
}

#[test]
fn test_iterator() {
    let name = random_table_name();

    let mut table = Table::<TestData>::create(&name, 5).unwrap();

    for i in 0..5 {
        let record = TestData {
            number: i,
            value: 100.0 / (i as f64),
        };
        table.insert(&record).expect("unable to insert record");
    }
    assert_eq!(table.count(), 5);
    assert_eq!(table.capacity(), 5);

    table.remove(3).expect("unable to remove record");

    let mut iter = table.iter();
    for i in 0..5 {
        if i == 3 {
            continue;
        }
        let expected = TestData {
            number: i,
            value: 100.0 / (i as f64),
        };
        let got = iter.next().expect("Iterator is none");
        assert_eq!(got, expected);
    }
    assert_eq!(iter.next(), None);
    assert_eq!(iter.next(), None);

    table.destroy().unwrap();

    assert!(!common::shm_exists(&name));
    assert!(!common::sem_exists(&name));
}

#[test]
fn test_keys_iterator() {
    let name = random_table_name();

    let mut table = Table::<TestData>::create(&name, 5).unwrap();

    for i in 0..5 {
        let record = TestData {
            number: i,
            value: 100.0 / (i as f64),
        };
        table.insert(&record).expect("unable to insert record");
    }
    assert_eq!(table.count(), 5);
    assert_eq!(table.capacity(), 5);

    table.remove(3).expect("unable to remove record");

    let keys = table.keys().collect::<Vec<_>>();
    assert_eq!(keys, vec![0, 1, 2, 4]);

    table.destroy().unwrap();

    assert!(!common::shm_exists(&name));
    assert!(!common::sem_exists(&name));
}

#[test]
fn test_update_many() {
    let name = random_table_name();

    let mut table = Table::<TestData>::create(&name, 5).unwrap();

    for i in 0..5 {
        let record = TestData {
            number: i,
            value: 100.0 / (i as f64),
        };
        table.insert(&record).expect("unable to insert record");
    }
    assert_eq!(table.count(), 5);
    assert_eq!(table.capacity(), 5);

    let count = table
        .update_many([1, 4, 10], |record| record.value *= 2.0)
        .expect("update_many");
    assert_eq!(count, 2);

    for i in 0..5 {
        let k = if i == 1 || i == 4 { 2.0 } else { 1.0 };
        let expected = TestData {
            number: i,
            value: k * (100.0 / (i as f64)),
        };
        let got = table
            .find(i)
            .unwrap_or_else(|_| panic!("Record with key {} not found", i))
            .unwrap_or_else(|| panic!("Record with key {} not found", i));
        assert_eq!(got, expected);
    }

    table.destroy().unwrap();

    assert!(!common::shm_exists(&name));
    assert!(!common::sem_exists(&name));
}
