use std::cmp::{min, max};
use crate::modes::IsMode;
use std::fmt;

#[derive(Debug)]
pub struct KucherovMode {
    s_param : i32,
}

impl KucherovMode {
    pub fn new(args : &[&str]) -> Self{
        if args.len() != 1{
            panic!("Expecting one numeric argument as Kucherov's S parameter!");
        }
        let s_param : i32 = args[0].parse()
            .expect("Couldn't interpret the argument as a number!");
        assert!(s_param >= 1, "Kucherov's S parameter needs to be >= 1");
        KucherovMode {s_param : s_param}
    }
}


impl fmt::Display for KucherovMode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Kucherov S={}", self.s_param)
    }
}

#[allow(unused_variables)]
impl IsMode for KucherovMode {
    fn get_guaranteed_extra_blocks(&self) -> i32 {
        self.s_param
    }

    fn get_fewest_suff_blocks(&self) -> i32{
        self.s_param
    }

    fn filter_func(&self, completed_blocks : i32, patt_blocks : i32, blind_blocks : i32) -> i32{
        min(
            completed_blocks,
            patt_blocks - self.s_param,
        )
    }

    fn get_block_lengths(&self, patt_len : i32, err_rate : f32, thresh : i32) -> Vec<i32>{
        let mut ls : Vec<i32> = Vec::new();
        if patt_len < thresh{
            ls.push(patt_len);
            return ls;
        }
        for l in thresh..patt_len+1{
            let f_len = l as f32;
            if (err_rate*(f_len-1.0)).ceil()
                < (err_rate*f_len).ceil() {
                ls.push(l);
            }
        }
        ls.push(patt_len+1);
        let k =
            max(1, (err_rate*(ls[0] as f32)).ceil() as i32) //when err_rate is 0 there is no block to start with. avoid this niche case
            + self.s_param - 1;
        let big_l : i32 = max(
            (((ls[0]-1) as f32)/(k as f32)).ceil() as i32,
            ls[0] - thresh,
        );
        let p : i32 =  ((ls[0]-1-big_l) as f32 / ((k-1) as f32)).floor() as i32;
        let first_half_len : i32 = p*(k-1)+big_l;
        let longer_blocks_in_first_half : i32 = ls[0]-1-first_half_len;

        let mut block_lengths = Vec::new();
        for _ in 0..(k-1-longer_blocks_in_first_half){
            //shorter PRIOR blocks
            block_lengths.push(p);
        }
        for _ in 0..longer_blocks_in_first_half{
            //longer prior blocks
            block_lengths.push(p+1);
        }
        //L block
        block_lengths.push(big_l);
        for i in 0..ls.len()-1 {
            //ANTERIOR blocks
            block_lengths.push(ls[i+1] - ls[i]);
        }
        assert_eq!(block_lengths[0..(k as usize)].iter().sum::<i32>(), ls[0] - 1);
        assert!(block_lengths[(k-1) as usize] >= ls[0] - thresh);
        block_lengths
    }

    fn candidate_condition(&self,
            generous_overlap_len : i32,
            completed_blocks : i32,
            thresh : i32,
            errors : i32
            ) -> bool{
        let c1 = generous_overlap_len >= thresh;
        let c2 = completed_blocks > 0;
        let c3 = completed_blocks >= self.s_param - 1
            &&
            errors <= (completed_blocks - self.s_param + 1);
        c1 && c2 && c3
    }
}
