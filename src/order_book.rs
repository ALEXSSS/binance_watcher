use crate::messages::{BookDepthUpdate, FullBook, LevelApi};
use std::cell::{Cell, RefCell};
use std::fmt::{Display, Formatter};

/// OrderBook maintained during application runtime.
/// My thoughts:
/// This struct is Send, so it safe to use it cross-await call as we do (not simultaneously)
#[derive(Default)]
pub struct OrderBook {
    last_update_id: Cell<u64>,
    levels: Cell<u32>,
    symbol: String,
    bid: RefCell<Vec<Level>>,
    ask: RefCell<Vec<Level>>,
    is_just_initialised: Cell<bool>,
}

/// My thoughts:
/// in real life scenario better to use tick size (u8), and qty (as long), so 5.0009 = (4, 50009) = 50009 * 10 ^ -4
/// but for this app to ease development f64 used
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Level {
    pub quantity: f64,
    pub price: f64,
}

impl OrderBook {
    pub fn new(levels: u32, symbol: String) -> Self {
        Self {
            levels: Cell::new(levels),
            symbol: symbol.clone(),
            ..Default::default()
        }
    }

    pub fn get_mid(&self) -> Option<f64> {
        let bid = self.get_best_bid();
        match bid {
            Ok(bid_val) => {
                let ask = self.get_best_ask();
                match ask {
                    Ok(ask_val) => Some((ask_val.price - bid_val.price) / 2.0 + bid_val.price),
                    Err(_) => None,
                }
            }
            Err(_) => None,
        }
    }

    pub fn get_spread(&self) -> Option<f64> {
        let bid = self.get_best_bid();
        match bid {
            Ok(bid_val) => {
                let ask = self.get_best_ask();
                match ask {
                    Ok(ask_val) => Some(ask_val.price - bid_val.price),
                    Err(_) => None,
                }
            }
            Err(_) => None,
        }
    }

    pub fn get_best_bid(&self) -> Result<Level, String> {
        let reference = &self.bid.borrow();
        let level_option: Option<&Level> = reference.get(0);
        match level_option {
            None => Err("empty bid".to_string()),
            Some(level) => Ok(level.clone()),
        }
    }

    pub fn get_best_ask(&self) -> Result<Level, String> {
        let reference = &self.ask.borrow();
        let level_option: Option<&Level> = reference.get(0);
        match level_option {
            None => Err("empty ask".to_string()),
            Some(level) => Ok(level.clone()),
        }
    }

    pub fn apply_full_book_from_http_api(&mut self, book: &FullBook) {
        self.last_update_id.set(book.last_update_id);
        self.is_just_initialised.set(true);

        // bid
        self.bid.get_mut().clear();
        for level in &book.bids {
            self.bid.get_mut().push(level_api_to_level(&level));
        }

        // ask
        self.ask.get_mut().clear();
        for level in &book.asks {
            self.ask.get_mut().push(level_api_to_level(&level));
        }

        self.trim()
    }

    // Result?
    pub fn apply_depth_book_update_from_websocket(&mut self, book: &BookDepthUpdate) -> bool {
        // for already applied updates from ws
        if self.is_update_applied(book) {
            return true;
        }
        // if book already too old, we need ask http api again
        if !self.is_eligible_for_update(book) {
            return false;
        }
        // check that previous final id was last_id
        if !self.is_just_initialised.get() && self.last_update_id.get() != book.pu {
            return false;
        }

        // update
        for level in &book.b {
            self.apply_bid(&level);
        }
        for level in &book.a {
            self.apply_ask(&level);
        }
        self.last_update_id.set(book.u);
        self.trim();

        true
    }

    // utils
    fn is_update_applied(&self, book_update: &BookDepthUpdate) -> bool {
        self.last_update_id.get() > book_update.u
    }

    fn is_eligible_for_update(&self, book_update: &BookDepthUpdate) -> bool {
        let last_update_id = self.last_update_id.get();
        book_update.U <= last_update_id && last_update_id <= book_update.u
    }

    fn apply_bid(&mut self, api_level: &LevelApi) {
        Self::do_apply_to_level(&mut self.bid, api_level, false)
    }
    fn apply_ask(&mut self, api_level: &LevelApi) {
        Self::do_apply_to_level(&mut self.ask, api_level, true)
    }

    fn do_apply_to_level(levels: &mut RefCell<Vec<Level>>, api_level: &LevelApi, ascending: bool) {
        let level_update = level_api_to_level(api_level);
        let result = Self::look_for_level(level_update.price, levels.borrow().as_ref(), ascending);
        match result {
            Ok(index) => {
                let levels = levels.get_mut();
                if Self::floats_equal(level_update.price, 0.0) {
                    // TBD: could be done much more efficiently
                    levels.remove(index);
                } else {
                    levels[index] = Level {
                        price: level_update.price,
                        quantity: level_update.quantity,
                    }
                }
            }
            Err(index) => {
                let levels = levels.get_mut();
                if Self::floats_equal(level_update.price, 0.0) {
                    // ignore
                } else {
                    levels.insert(
                        index,
                        Level {
                            price: level_update.price,
                            quantity: level_update.quantity,
                        },
                    );
                }
            }
        }
    }

    fn look_for_level(price: f64, levels: &Vec<Level>, ascending: bool) -> Result<usize, usize> {
        // TBD: in reality unnecessary for small levels limits <=100
        levels.binary_search_by(|level| {
            if ascending {
                level.price.total_cmp(&price)
            } else {
                price.total_cmp(&level.price)
            }
        })
    }

    fn floats_equal(a: f64, b: f64) -> bool {
        (a - b).abs() < f64::EPSILON
    }

    fn trim(&mut self) {
        self.bid.get_mut().truncate(self.levels.get() as usize);
        self.ask.get_mut().truncate(self.levels.get() as usize)
    }

    fn write_level(
        &self,
        f: &mut Formatter<'_>,
        level_bid: Option<&Level>,
        level_ask: Option<&Level>,
    ) {
        let empty_level = "|         ---          |";
        match level_bid {
            Some(level) => {
                write!(f, "|{:10}|{:10}|", level.quantity, level.price).unwrap();
            }
            None => {
                write!(f, "{}", empty_level).unwrap();
            }
        }
        write!(f, "     ").unwrap();
        match level_ask {
            Some(level) => {
                write!(f, "|{:10}|{:10}|\n", level.quantity, level.price).unwrap();
            }
            None => {
                write!(f, "{}\n", empty_level).unwrap();
            }
        }
    }
}

impl Display for OrderBook {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(
            f,
            "====         ORDER BOOK : {}              ====",
            self.symbol
        )?;
        writeln!(
            f,
            "|                mid: {:10}                  |",
            self.get_mid().unwrap_or(f64::NAN)
        )?;
        writeln!(f, "|         bid         |     |         ask         |")?;
        writeln!(f, "|   qty    |   price  |     |   qty    |  price   |")?;
        writeln!(f, "---------------------------------------------------")?;
        for index in 0..self.levels.get() as usize {
            let reference = &self.bid.borrow();
            let bid_level: Option<&Level> = reference.get(index);
            let reference = &self.ask.borrow();
            let ask_level: Option<&Level> = reference.get(index);
            self.write_level(f, bid_level, ask_level);
        }
        writeln!(f, "====            END ORDER BOOK                 ====")?;
        Ok(())
    }
}

fn level_api_to_level(api_level: &LevelApi) -> Level {
    Level {
        quantity: api_level.quantity.parse::<f64>().unwrap(),
        price: api_level.price.parse::<f64>().unwrap(),
    }
}

mod test {
    use super::*;

    #[test]
    fn get_best_bid_test() {
        let mut book = OrderBook::default();

        book.bid.get_mut().push(Level {
            quantity: 1.0,
            price: 20.0,
        });
        book.bid.get_mut().push(Level {
            quantity: 1.0,
            price: 19.0,
        });
        book.bid.get_mut().push(Level {
            quantity: 1.0,
            price: 18.0,
        });
        book.bid.get_mut().push(Level {
            quantity: 1.0,
            price: 17.0,
        });

        book.ask.get_mut().push(Level {
            quantity: 1.0,
            price: 21.0,
        });
        book.ask.get_mut().push(Level {
            quantity: 1.0,
            price: 22.0,
        });
        book.ask.get_mut().push(Level {
            quantity: 1.0,
            price: 23.0,
        });
        book.ask.get_mut().push(Level {
            quantity: 1.0,
            price: 24.0,
        });

        assert_eq!(
            book.get_best_bid().unwrap(),
            Level {
                quantity: 1.0,
                price: 20.0
            }
        );
        assert_eq!(
            book.get_best_ask().unwrap(),
            Level {
                quantity: 1.0,
                price: 21.0
            }
        );
    }

    #[test]
    fn apply_http_full_book_apply_test() {
        let mut book = OrderBook::default();
        book.levels.set(3);

        let http_book = FullBook {
            last_update_id: 100500,
            bids: vec![
                LevelApi {
                    quantity: "1".to_string(),
                    price: "5".to_string(),
                },
                LevelApi {
                    quantity: "1".to_string(),
                    price: "4".to_string(),
                },
                LevelApi {
                    quantity: "1".to_string(),
                    price: "3".to_string(),
                },
            ],
            asks: vec![
                LevelApi {
                    quantity: "1".to_string(),
                    price: "6".to_string(),
                },
                LevelApi {
                    quantity: "1".to_string(),
                    price: "7".to_string(),
                },
                LevelApi {
                    quantity: "1".to_string(),
                    price: "8".to_string(),
                },
            ],
        };

        book.apply_full_book_from_http_api(&http_book);

        assert_eq!(
            book.get_best_bid().unwrap(),
            Level {
                quantity: 1.0,
                price: 5.0
            }
        );
        assert_eq!(
            book.get_best_ask().unwrap(),
            Level {
                quantity: 1.0,
                price: 6.0
            }
        );
        assert_eq!(book.bid.borrow().len(), 3);
        assert_eq!(book.ask.borrow().len(), 3);

        // change levels param

        book.levels.set(2);
        book.apply_full_book_from_http_api(&http_book);

        assert_eq!(
            book.get_best_bid().unwrap(),
            Level {
                quantity: 1.0,
                price: 5.0
            }
        );
        assert_eq!(
            book.get_best_ask().unwrap(),
            Level {
                quantity: 1.0,
                price: 6.0
            }
        );
        assert_eq!(book.bid.borrow().len(), 2);
        assert_eq!(book.ask.borrow().len(), 2);
    }

    #[test]
    fn apply_websocket_update_book_apply_test() {
        let mut book = OrderBook::default();
        book.levels.set(3);
        book.is_just_initialised.set(true);

        let ws_book = BookDepthUpdate {
            e: "".to_string(),
            E: 0,
            T: 0,
            s: "".to_string(),
            U: 100000,
            u: 100500,
            pu: 0,
            b: vec![
                LevelApi {
                    quantity: "1".to_string(),
                    price: "5".to_string(),
                },
                LevelApi {
                    quantity: "1".to_string(),
                    price: "4".to_string(),
                },
                LevelApi {
                    quantity: "1".to_string(),
                    price: "3".to_string(),
                },
            ],
            a: vec![
                LevelApi {
                    quantity: "1".to_string(),
                    price: "6".to_string(),
                },
                LevelApi {
                    quantity: "1".to_string(),
                    price: "7".to_string(),
                },
                LevelApi {
                    quantity: "1".to_string(),
                    price: "8".to_string(),
                },
            ],
        };

        let succ = book.apply_depth_book_update_from_websocket(&ws_book);

        // 1) our original book is too old with last_update_id == 0, update should return false
        assert_eq!(succ, false);

        // 2) if book already applied update, then nothing should be done
        book.last_update_id.set(100501);

        let succ = book.apply_depth_book_update_from_websocket(&ws_book);

        assert_eq!(succ, true);
        assert_eq!(book.bid.borrow().len(), 0);
        assert_eq!(book.ask.borrow().len(), 0);
        assert_eq!(book.last_update_id.get(), 100501);

        // 3) we are in range for update between U <= last_update_id <= u

        book.last_update_id.set(100499);

        let succ = book.apply_depth_book_update_from_websocket(&ws_book);

        assert_eq!(succ, true);
        assert_eq!(book.bid.borrow().len(), 3);
        assert_eq!(book.ask.borrow().len(), 3);
        assert_eq!(book.last_update_id.get(), 100500);

        assert_eq!(
            book.get_best_bid().unwrap(),
            Level {
                quantity: 1.0,
                price: 5.0
            }
        );
        assert_eq!(
            book.get_best_ask().unwrap(),
            Level {
                quantity: 1.0,
                price: 6.0
            }
        );
    }
}
