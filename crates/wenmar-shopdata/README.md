# wenmar-shopdata

A Wenmar Open shop file is the second data file, `wenmar-open-shop-YYYY.MM.DD.sqlite3`, holding the datasets listed in its `datasets` meta value, such as recalls, each matched to the vehicle catalog. `ShopData::new` refuses a shop file of another schema version.
