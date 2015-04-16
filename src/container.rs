use std::io;
use docker;
use rustc_serialize::json;

pub fn get_containers_as_str() -> io::Result<String> {
    let docker = docker::Docker::new();
    let containers = match docker.get_containers(true) {
        Ok(containers) => containers,
        Err(e) => {
            println!("{}", e);
            let err = io::Error::new(io::ErrorKind::ConnectionAborted,
                                     "A connection to Docker is aborted.");
            return Err(err);
        }
    };

    let mut cosmos_containers: Vec<Container> = Vec::new();
    for container in containers.iter() {
        let stats = match docker.get_stats(&container) {
            Ok(stats) => stats,
            Err(e) => {
                println!("{}", e);
                let err = io::Error::new(io::ErrorKind::ConnectionAborted,
                                         "A connection to Docker is aborted.");
                return Err(err);
            }
        };

        let delayed_stats = match docker.get_stats(&container) {
            Ok(stats) => stats,
            Err(e) => {
                println!("{}", e);
                let err = io::Error::new(io::ErrorKind::ConnectionAborted,
                                         "A connection to Docker is aborted.");
                return Err(err);
            }
        };

        cosmos_containers.push(container.to_cosmos_container(&stats, &delayed_stats));
    }

    let encoded_cosmos_containers = match json::encode(&cosmos_containers) {
        Ok(s) => s,
        Err(e) => {
            println!("{}", e);
            let err = io::Error::new(io::ErrorKind::InvalidInput,
                                     "Encoding is failed for containers.");
            return Err(err);
        }
    };

    return Ok(encoded_cosmos_containers);
}

pub fn get_hostname() -> io::Result<String> {
    let docker = docker::Docker::new();

    let hostname = match docker.get_info() {
        Ok(info) => info.Name,
        Err(e) => {
            println!("{}", e);
            let err = io::Error::new(io::ErrorKind::NotConnected,
                                     "A connection to Docker is aborted.");
            return Err(err);
        }
    };

    return Ok(hostname);
}

trait CosmosContainerDecodable {
    fn to_cosmos_container(&self,
                           stats: &docker::stats::Stats,
                           delayed_stats: &docker::stats::Stats) -> Container;
}

impl CosmosContainerDecodable for docker::container::Container {
    fn to_cosmos_container(&self,
                           stats: &docker::stats::Stats,
                           delayed_stats: &docker::stats::Stats) -> Container {
        // network
        let network = Network {
            RxBytes: delayed_stats.network.rx_bytes,
            TxBytes: delayed_stats.network.tx_bytes,
            RxBytesDelta: delayed_stats.network.rx_bytes - stats.network.rx_bytes,
            TxBytesDelta: delayed_stats.network.tx_bytes - stats.network.tx_bytes
        };

        // memory
        let memory = Memory {
            Limit: delayed_stats.memory_stats.limit,
            Usage: delayed_stats.memory_stats.usage
        };

        // cpu
        let cpus = stats.cpu_stats.cpu_usage.percpu_usage.len();

        let total_usage = stats.cpu_stats.cpu_usage.total_usage;
        let delayed_total_usage = delayed_stats.cpu_stats.cpu_usage.total_usage;
        let system_usage = stats.cpu_stats.system_cpu_usage;
        let delayed_system_usage = delayed_stats.cpu_stats.system_cpu_usage;
        let total_percent = get_cpu_percent(total_usage,
                                            delayed_total_usage,
                                            system_usage,
                                            delayed_system_usage,
                                            cpus);

        let mut percpus: Vec<f64> = Vec::new();
        for i in 0..cpus {
            let val = stats.cpu_stats.cpu_usage.percpu_usage[i];
            let delayed_val = delayed_stats.cpu_stats.cpu_usage.percpu_usage[i];
            let percent = get_cpu_percent(val,
                                          delayed_val,
                                          system_usage,
                                          delayed_system_usage,
                                          cpus);
            percpus.push(percent);
        }

        let cpu = Cpu {
            TotalUtilization: total_percent,
            PerCpuUtilization: percpus
        };

        // stats
        let stats = Stats {
            Network: network,
            Cpu: cpu,
            Memory: memory
        };

        // names
        let mut names: Vec<String> = Vec::new();
        for name in self.Names.iter() {
            let is_contained = name.as_bytes()[0] == "/".as_bytes()[0];
            match is_contained {
                true => {
                    let mut index = 0;
                    let mut new_name: Vec<u8> = Vec::new();
                    for b in name.as_bytes() {
                        index += 1;
                        if index == 1 { continue; }
                        new_name.push(*b);
                    }
                    names.push(String::from_utf8(new_name).unwrap());
                }
                false => { names.push(name.clone()); }
            };
        }

        // container
        let container = Container {
            Id: self.Id.clone(),
            Image: self.Image.clone(),
            Status: self.Status.clone(),
            Command: self.Command.clone(),
            Created: self.Created.clone(),
            Names: names,
            Ports: self.Ports.clone(),
            Stats: stats
        };

        return container;
    }
}

fn get_cpu_percent(cpu_val: u64,
                   delayed_cpu_val: u64,
                   system_val: u64,
                   delayed_system_val: u64,
                   cpus: usize) -> f64 {
    let cpu_val_delta: f64 = (delayed_cpu_val - cpu_val) as f64;
    let system_val_delta: f64 = (delayed_system_val - system_val) as f64;
    let mut percent = (cpu_val_delta / system_val_delta) * cpus as f64 * 100.0 as f64;
    if percent <= 0.0 { percent = 0.0; }
    return percent;
}

#[derive(RustcEncodable, RustcDecodable)]
#[allow(non_snake_case)]
struct Container {
    Id: String,
    Image: String,
    Status: String,
    Command: String,
    Created: u64,
    Names: Vec<String>,
    Ports: Vec<docker::container::Port>,
    Stats: Stats
}

#[derive(RustcEncodable, RustcDecodable)]
#[allow(non_snake_case)]
struct Stats {
    Network: Network,
    Cpu: Cpu,
    Memory: Memory
}

#[derive(RustcEncodable, RustcDecodable)]
#[allow(non_snake_case)]
struct Network {
    RxBytes: u64,
    TxBytes: u64,
    RxBytesDelta: u64,
    TxBytesDelta: u64
}

#[derive(RustcEncodable, RustcDecodable)]
#[allow(non_snake_case)]
struct Cpu {
    TotalUtilization: f64,
    PerCpuUtilization: Vec<f64>
}

#[derive(RustcEncodable, RustcDecodable)]
#[allow(non_snake_case)]
struct Memory {
    Limit: u64,
    Usage: u64
}