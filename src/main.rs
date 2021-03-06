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
            .arg(Arg::with_name("gpuclock")
                .long("gpuclock")
                .multiple(true)
                .takes_value(true)
                .help("GPU clock offset (kHz)"))
            .arg(Arg::with_name("memclock")
                .long("memclock")
                .multiple(true)
                .takes_value(true)
                .help("Memory clock offset (kHz)"))
            .arg(Arg::with_name("plimit")
                .long("plimit")
                .multiple(true)
                .takes_value(true)
                .help("Power limit (%)"))
            .arg(Arg::with_name("tlimit")
                .long("tlimit")
                .multiple(true)
                .takes_value(true)
                .help("Temperature limit (deg C)"))
            .arg(Arg::with_name("vlock")
                .long("vlock")
                .multiple(true)
                .takes_value(true)
                .help("Lock voltage (uV)")))
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

            let gpuclock = parse_arg::<i32>(inner_matches, "gpuclock", selected_gpus.len());
            let memclock = parse_arg::<i32>(inner_matches, "memclock", selected_gpus.len());
            let plimit = parse_arg::<u32>(inner_matches, "plimit", selected_gpus.len());
            let tlimit = parse_arg::<i32>(inner_matches, "tlimit", selected_gpus.len());
            let vlock = parse_arg::<u32>(inner_matches, "vlock", selected_gpus.len());

            for (i, (global_idx, gpu)) in selected_gpus.iter().enumerate() {
                // gpu clock
                // TODO: validate using gpu.inner().vfp_ranges()
                match &gpuclock {
                    Some(gpuclock) => {
                        let delta = KilohertzDelta(gpuclock[i]);

                        println!("Setting GPU #{} graphics clock to {:?}", global_idx, delta);

                        gpu.inner().set_pstates([(PState::P0, ClockDomain::Graphics, delta)].iter().cloned()).unwrap();
                    },
                    None => ()
                };
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
                // power limit
                // TODO: validate using gpu.inner().power_limit_info()
                match &plimit {
                    Some(plimit) => {
                        let power_limit = Percentage(plimit[i]);

                        println!("Setting GPU #{} power limit to {:?}", global_idx, plimit[i]);

                        let power_vec = vec![power_limit];
                        gpu.set_power_limits(power_vec.iter().cloned()).unwrap();
                    },
                    None => ()
                };
                // temp limit
                // TODO: validate using gpu.inner().thermal_limit_info()
                match &tlimit {
                    Some(tlimit) => {
                        let (_, controllers) = gpu.inner().thermal_limit_info().unwrap();
                        let temp_limit = Celsius(tlimit[i]);

                        println!("Setting GPU #{} temperature limit to {:?}", global_idx, temp_limit);

                        let temps_vec = vec![temp_limit; controllers.len() as usize];
                        gpu.set_sensor_limits(temps_vec.iter().cloned()).unwrap();
                    },
                    None => ()
                };
                // voltage lock
                match &vlock {
                    Some(voltages) => {
                        let value = Microvolts(voltages[i]);

                        println!("Setting GPU #{} lock voltage to {:?}", global_idx, value);

                        gpu.set_vfp_lock(value).unwrap();
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

                let info: GpuInfo = gpu.info().unwrap();

                // power limit
                gpu.set_power_limits(info.power_limits.iter().map(|pl: &PowerLimit| pl.default)).unwrap();

                // temp limit
                gpu.set_sensor_limits(info.sensor_limits.iter().map(|pl: &SensorLimit| pl.default)).unwrap();

                // voltage lock
                gpu.reset_vfp_lock().unwrap();
            }
        },
        ("", ..) => (),
        _ => unreachable!("unknown command"),
    }
}
