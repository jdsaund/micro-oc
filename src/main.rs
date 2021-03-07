extern crate num_traits;
extern crate nvapi_hi as nvapi;

use cli_table::{format::Justify, print_stdout, Cell, Style, Table};
use num_traits::Num;
use clap::{
    Arg,
    ArgMatches,
    App,
    Error,
    ErrorKind,
    SubCommand
};
use nvapi::{
    Celsius,
    ClockDomain,
    Gpu,
    GpuInfo,
    KilohertzDelta,
    Microvolts,
    PState,
    Percentage,
    PowerLimit,
    SensorLimit,
    Status
};

fn parse_arg<T: Num> (matches: &ArgMatches, param: &str, expected_len: usize) -> Option<Vec<T>> {
    match matches.values_of(param) {
        Some(values) => {
            let values = values.map(|v| T::from_str_radix(v, 10).ok().unwrap()).collect::<Vec<T>>();
            if values.len() != expected_len {

                let error = Error {
                    message: format!("Wrong number of '{}' values, got {}, expected {}", param, values.len(), expected_len),
                    kind: ErrorKind::WrongNumberOfValues,
                    info: None
                };

                error.exit();
            }

            Some(values)
        },
        None => None
    }
}

fn select_gus <'a>(gpus: &'a Vec<Gpu>, matches: &ArgMatches) -> Vec<(usize, &'a Gpu)> {
    let selected_ids: Vec<(usize, &'a Gpu)> = matches.values_of("ids")
        .unwrap()
        .map(|val| {
            let idx = usize::from_str_radix(val, 10).unwrap();
            (idx, &gpus[idx])
        })
        .collect::<Vec<(usize, &'a Gpu)>>();

    selected_ids
}

fn main() -> () {
    let matches = App::new("micro-oc")
        .arg(Arg::with_name("ids")
            .multiple(true)
            .takes_value(true)
            .help("The list of GPU indexes, space separated"))
        .subcommand(SubCommand::with_name("set")
            .arg(Arg::with_name("memclock")
                .long("memclock")
                .multiple(true)
                .takes_value(true)
                .help("Memory clock offset (kHz)")))
        .subcommand(SubCommand::with_name("list"))
        .subcommand(SubCommand::with_name("reset"))
        .get_matches();

    nvapi::initialize().unwrap();

    let gpus = Gpu::enumerate().unwrap();
    let info: Vec<GpuInfo> = gpus.iter()
        .map(|gpu: &Gpu| Ok::<GpuInfo, Status>(gpu.info().unwrap()))
        .collect::<Result<Vec<GpuInfo>, Status>>()
        .unwrap();

    match matches.subcommand() {
        ("list", Some(..)) => {
            let table = info.iter()
                .zip(gpus.iter())
                .enumerate()
                .map(|(i, (info, gpu))| {
                    vec![
                        format!("GPU #{}", i).cell().justify(Justify::Left),
                        info.name.clone().cell().justify(Justify::Left),
                        info.vendor.clone().cell().justify(Justify::Left),
                        gpu.inner().gpu_id().unwrap().cell().justify(Justify::Right),
                    ]
                })
                .table()
                .title(vec![
                    "GPU Index".cell().bold(true),
                    "Name".cell().bold(true),
                    "Vendor".cell().bold(true),
                    "Device ID".cell().bold(true),
                ]);

            assert!(print_stdout(table).is_ok());
        },
        ("set", Some(inner_matches)) => {
            let selected_gpus = select_gus(&gpus, &matches);

            let memclock = parse_arg::<i32>(inner_matches, "memclock", selected_gpus.len());

            for (i, (global_idx, gpu)) in selected_gpus.iter().enumerate() {
                // memory clock
                // TODO: validate using gpu.inner().vfp_ranges()
                match &memclock {
                    Some(memclock) => {
                        let delta = KilohertzDelta(memclock[i]);

                        println!("Setting GPU #{} memory clock to {:?}", global_idx, delta);

                        gpu.inner().set_pstates([(PState::P0, ClockDomain::Memory, delta)].iter().cloned()).unwrap();
                    },
                    None => ()
                };
            }
        },
        ("reset", Some(..)) => {
            for (i, gpu) in gpus.iter().enumerate() {
                println!("Resetting GPU #{}", i);

                // clocks
                let deltas = [
                    (PState::P0, ClockDomain::Graphics, KilohertzDelta(0)),
                    (PState::P0, ClockDomain::Memory, KilohertzDelta(0)),
                ].iter().cloned();

                gpu.inner().set_pstates(deltas).unwrap();
            }
        },
        ("", ..) => (),
        _ => unreachable!("unknown command"),
    }
}
