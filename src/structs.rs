
/*
Structs and methods for solutions and candidate solutions to be used throughout the program
*/
pub mod solutions{
    use std::hash::{Hash, Hasher};
    use std::cmp::Ordering;
    use std::fmt;
    use std::cmp::max;
    use std::mem::swap;

    // Normal refers to a solution where both strings are NOT reversed
    // Reversed refers to a solution where A is normal and B is reversed
    #[derive(Hash,PartialEq, Eq, Debug, PartialOrd, Ord)]
    pub enum Orientation{
        Normal,
        Reversed,
    }

    impl fmt::Display for Orientation {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            let s = if *self == Orientation::Normal{"N"} else {"I"};
            write!(f, "{}", s)
        }
    }

    //NOT oriented
    #[derive(Hash,PartialEq, Eq, Debug)]
    pub struct Candidate{
        pub id_b : usize,
        pub overlap_a : usize,
        pub overlap_b : usize,
        pub overhang_left_a : i32,

        //DEBUG
        pub debug_str : String,
    }

    impl Candidate{

        #[inline]
        pub fn a1(&self) -> usize {
            max(0, self.overhang_left_a) as usize
        }

        #[inline]
        pub fn b1(&self) -> usize {
            max(0, -self.overhang_left_a) as usize
        }

        #[inline]
        pub fn a2(&self) -> usize {
            self.overlap_a
        }

        #[inline]
        pub fn b2(&self) -> usize {
            self.overlap_b
        }

        //TODO tidy up these silly a2() etc. calls later
        #[inline]
        pub fn a3(&self, a_len : usize) -> usize {
            assert!(a_len >= self.a1() + self.overlap_a);
            a_len - self.a1() - self.a2()
        }

        #[inline]
        pub fn b3(&self, b_len : usize) -> usize {
            assert!(b_len >= self.b1() + self.overlap_b);
            b_len - self.b1() - self.b2()
        }
    }

    //oriented
    #[derive(Debug)]
    pub struct Solution{
        pub id_a : usize,
        pub id_b : usize,
        pub orientation : Orientation,
        pub overhang_left_a : i32,
        pub overhang_right_b : i32,
        pub overlap_a : usize,
        pub overlap_b : usize,
        pub errors : u32,
        pub cigar : String,
    }

    impl Solution{
        pub fn v_flip(&mut self){
            self.overhang_left_a *= -1;
            self.overhang_right_b *= -1;
            swap(&mut self.id_a, &mut self.id_b);
            swap(&mut self.overlap_a, &mut self.overlap_b);
            //VFLIP CIGAR
        }

        pub fn un_reverse(&mut self){
            swap(&mut self.overhang_left_a, &mut self.overhang_right_b);
            self.overhang_left_a *= -1;
            self.overhang_right_b *= -1;
            //H-FLIP CIGAR
        }
    }

    impl Ord for Solution {
        fn cmp(&self, other: &Self) -> Ordering {
            (self.id_a, self.id_b, &self.orientation, self.overhang_left_a, self.overhang_right_b, self.overlap_a, self.overlap_b)
                .cmp(&(other.id_a, other.id_b, &other.orientation, other.overhang_left_a, other.overhang_right_b, other.overlap_a, other.overlap_b))
        }
    }

    impl PartialOrd for Solution {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            Some(self.cmp(other))
        }
    }

    impl PartialEq for Solution {
        fn eq(&self, other: &Self) -> bool {
            (self.id_a, self.id_b, &self.orientation, self.overhang_left_a, self.overhang_right_b, self.overlap_a, self.overlap_b)
                == (other.id_a, other.id_b, &other.orientation, other.overhang_left_a, other.overhang_right_b, other.overlap_a, other.overlap_b)
        }
    }

    impl Eq for Solution { }

    impl Hash for Solution {
        fn hash<H: Hasher>(&self, state: &mut H) {
            self.id_a.hash(state);
            self.id_b.hash(state);
            self.orientation.hash(state);
            self.overlap_a.hash(state);
            self.overlap_b.hash(state);
            self.overhang_left_a.hash(state);
            self.overhang_right_b.hash(state);
            // ERRORS and CIGAR not need to contribute to uniqueness
        }
    }
}


/*
Some structs and convenience functions for storing the data needed throughout the run.
The config struct stores the user's input parameters and is checked frequently but never changed.
The maps struct is built once, stores text, and various mappings between different string
representations and is queried throughout the run, also never changing after being populated.
*/
pub mod run_config{
    extern crate bidir_map;
    use bidir_map::BidirMap;
    use std;

    #[derive(Debug)]
    pub struct Maps{
        pub text : Vec<u8>,
        pub id2name_vec : Vec<String>,
        pub id2index_bdmap : BidirMap<usize, usize>,
        pub num_ids : usize,
    }

    impl Maps{

        pub fn get_string(&self, id : usize) -> &[u8]{
            assert!(id < self.num_ids);
            &self.text[*self.id2index_bdmap.get_by_first(&id).expect("GAH")..self.get_end_index(id)]
        }

        pub fn get_length(&self, id : usize) -> usize{
            assert!(id < self.num_ids);
            self.get_end_index(id) - self.id2index_bdmap.get_by_first(&id).expect("WOO")
        }

        fn get_end_index(&self, id : usize) -> usize{
            assert!(id < self.num_ids);
            if id == self.num_ids-1{
                self.text.len() - 1 //$s in front. one # at the end
            }else{
                self.id2index_bdmap.get_by_first(&(id + 1)).expect("WAHEY") - 1
            }
        }

        //returns (id, index)
        pub fn find_occurrence_containing(&self, index : usize) -> (usize, usize){
            let mut best = (0, 1);
            for &(id, ind) in self.id2index_bdmap.iter(){
                if ind <= index && ind > best.1{
                    best = (id, ind);
                }
            }
            best
        }

        pub fn get_name_for(&self, id : usize) -> &str {
            self.id2name_vec.get(id).expect("get name")
        }

        pub fn print_text_debug(&self){
            println!("{}", String::from_utf8_lossy(&self.text));
        }

        pub fn formatted(&self, id : usize) -> String{
            format!("{}",String::from_utf8_lossy(self.get_string(id)))
        }

        pub fn push_string(&self, print : &str, push_str : &str, pushes : usize) -> String{
            let mut s = String::new();
            for _ in 0..pushes{
                s.push_str(push_str);
            }
            s.push_str(print);
            s
        }

        #[inline]
        pub fn id_for(&self, id : usize) -> usize{
            *(self.id2index_bdmap.get_by_second(&id)
                .expect(&format!("no index for ID {}. input has IDs from 0 --> {}",
                                id, self.num_ids)))
        }

        #[inline]
        pub fn index_for(&self, index : usize) -> usize{
            *(self.id2index_bdmap.get_by_first(&index)
                .expect(&format!("no id at index {}", index)))
        }
    }

    pub static N_ALPH : &'static [u8] = b"ACGNT";
    pub static ALPH : &'static [u8] = b"ACGT";
    #[derive(Debug)]
    pub struct Config{
        //TODO benchmark argument

        //required
        pub input : String,
        pub output : String,
        pub err_rate : f32,
        pub thresh : i32,
        pub worker_threads: usize,

        //optional
        pub sorted : bool,
        pub reversals : bool,
        pub inclusions : bool,
        pub edit_distance : bool,
        pub verbose : bool,
        pub time: bool,
        pub print: bool,
        pub n_alphabet: bool,
    }

    impl Config{
        pub fn alphabet(&self) -> &[u8]{
            if self.n_alphabet {
                &N_ALPH
            } else {
                &ALPH
            }
        }
    }
}
