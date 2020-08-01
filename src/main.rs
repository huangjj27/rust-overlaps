use bio::data_structures::bwt::{DerefBWT, DerefOcc, DerefLess};
use bio::data_structures::bwt::{bwt, less, Occ};
use bio::data_structures::fmindex::FMIndex;
use bio::data_structures::suffix_array::suffix_array;
use bio::data_structures::suffix_array::RawSuffixArray;
use bio::alphabets::Alphabet;
use std::fs::File;
use std::io::{Write, BufWriter};
use std::collections::HashSet;
use std::time::Instant;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::{thread, time};
use std::io::stdout;

////////////////////////////////////////////////////////////////////////

mod setup;
mod prepare;
mod search;
mod verification;
mod structs;
mod modes;
mod testing;
mod useful;

use crate::structs::solutions::Solution;
use crate::structs::run_config::{Config, Maps};
use crate::search::GeneratesCandidates;
use crate::modes::Mode;

pub static READ_ERR : u8 = b'N';
static ATOMIC_TASKS_DONE: AtomicUsize = AtomicUsize::new(0);

/*
Gets the config and writes all the necessary data into the map struct.
calls solve() which does all the work
*/
fn main() {
    let (mode, config) = setup::parse_run_args();
    if config.verbosity >= 2 {
        println!("OK interpreted config args.\n{:#?}", &config);
        println!("OK mode set to {}", &mode);
    }
    let maps = prepare::read_and_prepare(&config.input, &config)
        .expect("Couldn't interpret data.");
    if config.verbosity >= 2 {
        println!("OK read and mapped fasta input.");
        if !config.n_alphabet{
            println!("OK cleaned 'N' from input strings.");
        }
    };
    solve(&config, &maps, mode);
}

/*
1. build index from text
2. prepare output file
3. generate tasks for each FORWARD string in the text (ie: patterns)
4. spawn workers in a threadpool to solve tasks
5. write to output either after verification
*/
fn solve(config : &Config, maps : &Maps, mode : Mode){
    let alphabet = Alphabet::new(config.alphabet());
    if config.verbosity >= 2 {
        println!("OK index alphabet set to '{}'",
                 String::from_utf8_lossy(config.alphabet()));
    }
    let sa = suffix_array(&maps.text);
    let bwt = bwt(&maps.text, &sa);
    let less = less(&bwt, &alphabet);
    let occ = Occ::new(&bwt, 3, &alphabet);
    let fm = FMIndex::new(&bwt, &less, &occ);
    if config.verbosity >= 2 {println!("OK index ready.");};

    let f = File::create(&config.output)
        .expect("Couldn't open output file.");
    let mut wrt_buf = BufWriter::new(f);
    if config.format_line{
        wrt_buf.write_all("idA\tidB\tO\tOHA\tOHB\tOLA\tOLB\tK\n".as_bytes())
            .expect("couldn't write header line to output");
        if config.verbosity >= 2 {println!("OK wrote header line to output file.");}
    }
    if config.verbosity >= 2 {println!("OK output writer ready.");}

    let id_iterator = 0..maps.num_ids();
    let mut complete_solution_list : Vec<Solution> = Vec::new(); // used when -g is not used
    let config_task_completion_clone = config.track_progress;
    let num_tasks = maps.num_ids();

    let progress_tracker = thread::spawn(move || {
        track_progress(config_task_completion_clone, num_tasks);
    }); // spawn progress-tracker thread
    if config.track_progress {
        if config.verbosity >= 2 {println!("OK spawning progress tracker thread.");}
    }else{
        if config.verbosity >= 2 {println!("OK suppressing progress tracker thread.");}
    }
    if config.verbosity >= 2 {println!("OK spawning {} worker threads.", config.worker_threads);}

    if config.verbosity >= 1{
        println!("OK working.");
    }
    let work_start = Instant::now();
    { //borrow block for solution set
        let computation = |id_a|  solve_an_id(config, maps, id_a, &sa, &fm, &mode);
        let aggregator = |solutions| {               // aggregation to apply to work results
            if config.greedy_output {
                //workers ==> out
                for sol in solutions {write_solution(&mut wrt_buf, &sol, maps, config);}
                wrt_buf.flush().is_ok();
            }else {
                //workers ==> solutions --> sorted_solutions --> out
                for sol in solutions {&mut complete_solution_list.push(sol);}
            }
            if config.track_progress { ATOMIC_TASKS_DONE.fetch_add(1, Ordering::SeqCst);}
        };
        cue::pipeline(
            "overlap_pipeline",          // name of the pipeline for logging
             config.worker_threads,      // number of worker threads
             id_iterator,                // iterator with work items
             computation,
             aggregator,
        );
    } // borrow of solution now returned

    if config.track_progress {
        ATOMIC_TASKS_DONE.store(num_tasks, Ordering::Relaxed);
        progress_tracker.join().is_ok();
    }

    if !config.greedy_output {
        complete_solution_list.sort_by(|a, b| solution_comparator(a, b, maps));
        if config.verbosity >= 2 {println!("OK output list sorted.");}
        complete_solution_list.dedup_by(|x, y| solution_comparator(x, y, maps) == std::cmp::Ordering::Equal);
        if config.verbosity >= 2 {println!("OK output list deduplicated.");}
        for sol in complete_solution_list.iter(){
            write_solution(&mut wrt_buf, sol, maps, config);
        }
        if config.verbosity >= 1{
            println!("OK wrote {} solutions.", complete_solution_list.len());
        }
    }
    if config.verbosity >= 2 {println!("OK output file {} written.", config.output);};
    if config.verbosity >= 1{
        println!("OK completed in {}.", approx_elapsed_string(&work_start));
    }
}


pub fn solution_comparator(x : &Solution, y : &Solution, maps : &Maps) -> std::cmp::Ordering{
    (maps.get_name_for(x.id_a), maps.get_name_for(x.id_b), &x.orientation, x.overhang_left_a, x.overhang_right_b, x.overlap_a, x.overlap_b)
        .cmp(&(maps.get_name_for(y.id_a), maps.get_name_for(y.id_b), &y.orientation, y.overhang_left_a, y.overhang_right_b, y.overlap_a, y.overlap_b))

}


fn approx_elapsed_string(start_time : &Instant) -> String{
    time_display(Instant::elapsed(&start_time).as_secs())
}


/*
If the user enables it, this time keeper process will print a nice progress bar
and ETA to STDOUT using carriage returns.
*/
fn track_progress(enabled : bool, num_tasks : usize) {
    let my_start_time = Instant::now();
    if !enabled {
        return;
    }
    let chars = 30;
    let mut complete = String::new();
    let mut incomplete = String::new();
    for _ in 0..chars{ incomplete.push(' ');}
    let sleep_time = time::Duration::from_millis(500);
    let mut tick_modulo = 0;
    let tick_out_freq = 8;
    let mut redraw = true;

    loop{
        let tasks_done = ATOMIC_TASKS_DONE.load(Ordering::Relaxed);
        while  tasks_done as f32 / (num_tasks as f32) > complete.len() as f32/ ((complete.len() + incomplete.len()) as f32){
            redraw = true;
            tick_modulo = -1;
            incomplete.pop();
            complete.push('#');
        }
        tick_modulo += 1;
        if redraw || tick_modulo == tick_out_freq {
            let elapsed = Instant::elapsed(&my_start_time).as_secs();
            let eta = elapsed as f32 * ((num_tasks-tasks_done) as f32) / (tasks_done as f32 + 0.2);
            let eta_str = time_display(eta as u64);
            print!("\r[{}{}] {}/{} tasks done. ETA {}                     ",
                   &complete, &incomplete, tasks_done, num_tasks, eta_str);
            stdout().flush().is_ok();
            redraw = false;
            tick_modulo = 0;
        }
        if tasks_done >= num_tasks{
            println!("\r[{}{}] {}/{} tasks done.                            ",
                     &complete, &incomplete, tasks_done, num_tasks);
            stdout().flush().is_ok();
            break;
        }
        thread::sleep(sleep_time);
    }
}


fn time_display(sec : u64) -> String{
    match sec {
        x if x == 0 => format!("< 1 sec"),
        x if x < 200 => format!("~{} sec", x),
        x if x < 60*120 => format!("~{} min", x/60),
        x if x < 60*60*100 => format!("~{} hrs", x/60/60),
        x if x < 60*60*24*3 => format!("~{} days", x/60/60/24),
        x if x < 60*60*24*7*3 => format!("~{} weeks", x/60/60/24/7),
        x if x < 60*60*24*30*4 => format!("~{} months", x/60/60/24/30),
        x if x < 60*60*24*365*5 => format!("~{} years", x/60/60/24/30),
        _ => format!("eternity"),
    }
}

/*
This is one task.
essentially converts an ID (and some constant information)
into a set of solutions involved with that ID.
*/
#[inline]
fn solve_an_id<DBWT: DerefBWT + Clone, DLess: DerefLess + Clone, DOcc: DerefOcc + Clone>
        (config : &Config, maps : &Maps, id_a : usize, sa : &RawSuffixArray,
         fm : &FMIndex<DBWT, DLess, DOcc>, mode : &Mode)
                -> HashSet<Solution>{
    let candidates = fm.generate_candidates(maps.get_string(id_a), config, maps, id_a, sa, mode);
    let solutions = verification::verify_all(id_a, candidates, config, maps);
    solutions
}


/*
writes a single solution to file.
the written string won't be broken up
*/
#[inline]
fn write_solution(buf : &mut BufWriter<File>, s : &Solution, maps : &Maps, config : &Config){
    let formatted = format!("{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
                            maps.get_name_for(s.id_a),
                            maps.get_name_for(s.id_b),
                            s.orientation,
                            s.overhang_left_a,
                            s.overhang_right_b,
                            s.overlap_a,
                            s.overlap_b,
                            s.errors,
    );
    buf.write(formatted.as_bytes()).is_ok();
    if config.print{
        let a = &String::from_utf8_lossy(maps.get_string(s.id_a));
        let b = &String::from_utf8_lossy(maps.get_string(s.id_b));
        let a_name = maps.get_name_for(s.id_a);
        let b_name = maps.get_name_for(s.id_b);
        if s.overhang_left_a > 0{
            let space = &std::iter::repeat(" ").take(s.overhang_left_a as usize).collect::<String>();
            println!(" '{}':\t{}\n '{}':\t{}{}\n", a_name, a, b_name, space, b);
        }else{
            let space = &std::iter::repeat(" ").take((-s.overhang_left_a) as usize).collect::<String>();
            println!(" '{}':\t{}{}\n '{}':\t{}\n", a_name, space, a, b_name, b);
        }
    }
}

impl<DBWT: DerefBWT + Clone, DLess: DerefLess + Clone, DOcc: DerefOcc + Clone> GeneratesCandidates
                    for FMIndex<DBWT, DLess, DOcc> {
    //empty
}
