// dbc-codegen output is not clippy clean
#![allow(clippy::all, clippy::pedantic, clippy::nursery)]
mod abs_alliance_can_messages;

pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
