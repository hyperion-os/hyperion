#![no_std]

use core::{
    fmt,
    ops::{Deref, DerefMut},
};

use hyperion_log::error;
use x86_64::instructions::port::Port;

//

pub fn devices() -> impl Iterator<Item = Device> {
    (0u8..=255)
        .flat_map(|bus| (0u8..32).map(move |device| (bus, device)))
        .flat_map(|(bus, device)| {
            let vendor = pci_config_read_word(bus, device, 0, 0);
            if vendor == 0xFFFF {
                return None;
            }

            let header_type = header_type(bus, device);
            if header_type & 0x80 == 0 {
                return Some(Either::L(
                    [DeviceLocation {
                        bus,
                        device,
                        func: 0,
                    }]
                    .into_iter(),
                ));
            }

            Some(Either::R((0u8..8).map(move |func| DeviceLocation {
                bus,
                device,
                func,
            })))
        })
        .flatten()
        .filter_map(|location @ DeviceLocation { bus, device, func }| {
            let vendor_id = pci_config_read_word(bus, device, func, 0);
            if vendor_id == 0xFFFF {
                return None;
            }

            let device_id = pci_config_read_word(bus, device, func, 2);
            let progif_rev = pci_config_read_word(bus, device, func, 8);
            let class_subclass = pci_config_read_word(bus, device, func, 10);

            Some(Device {
                location,
                vendor_id,
                device_id,
                class: (class_subclass >> 8) as u8,
                subclass: class_subclass as u8,
                prog_if: (progif_rev >> 8) as u8,
                rev_id: progif_rev as u8,
            })
        })
}

//

#[derive(Debug, Clone, Copy)]
pub struct DeviceLocation {
    pub bus: u8,
    pub device: u8,
    pub func: u8,
}

#[derive(Debug, Clone, Copy)]
pub struct Device {
    pub location: DeviceLocation,

    pub vendor_id: u16,
    pub device_id: u16,

    pub class: u8,
    pub subclass: u8,
    pub prog_if: u8,
    pub rev_id: u8,
}

impl Device {
    pub fn class_name(&self) -> &str {
        match self.class {
            0x0 => "Unclassified",
            0x1 => "Mass Storage Controller",
            0x2 => "Network Controller",
            0x3 => "Display Controller",
            0x4 => "Multimedia Controller",
            0x5 => "Memory Controller",
            0x6 => "Bridge",
            0x7 => "Simple Communication Controller",
            0x8 => "Base System Peripheral",
            0x9 => "Input Device Controller",
            0xA => "Docking Station",
            0xB => "Processor",
            0xC => "Serial Bus Controller",
            0xD => "Wireless Controller",
            0xE => "Intelligent Controller",
            0xF => "Satellite Communication Controller",
            0x10 => "Encryption Controller",
            0x11 => "Signal Processing Controller",
            0x12 => "Processing Accelerator",
            0x13 => "Non-Essential Instrumentation",
            0x14..=0x3F => "(Reserved)",
            0x40 => "Co-Processor",
            0x41..=0xFE => "(Reserved)",
            0xFF => "Unassigned Class (Vendor specific)",
        }
    }

    pub fn subclass_name(&self) -> &str {
        match (self.class, self.subclass) {
            (0x0, 0x0) => "VGA incompatible controller unclassified device",
            (0x0, 0x1) => "VGA incompatible controller unclassified device",
            (0x0, _) => "Unknown unclassified device",

            (0x1, 0x0) => "SCSI bus controller",
            (0x1, 0x1) => "IDE controller",
            (0x1, 0x2) => "Floppy disk controller",
            (0x1, 0x3) => "IPI bus controller",
            (0x1, 0x4) => "RAID controller",
            (0x1, 0x5) => "ATA controller",
            (0x1, 0x6) => "SATA controller",
            (0x1, 0x7) => "Serial attached SCSI controller",
            (0x1, 0x8) => "Non-Volatile memory controller",
            (0x1, 0x80) => "Other storage controller",
            (0x1, _) => "Unknown storage controller",

            (0x2, 0x0) => "Ethernet controller",
            (0x2, 0x1) => "Token ring controller",
            (0x2, 0x2) => "FDDI controller",
            (0x2, 0x3) => "ATM controller",
            (0x2, 0x4) => "ISDN controller",
            (0x2, 0x5) => "WorldFip controller",
            (0x2, 0x6) => "PICMG 2.14 multi-computing controller",
            (0x2, 0x7) => "Infiniband controller",
            (0x2, 0x8) => "Fabric controller",
            (0x2, 0x80) => "Other network controller",
            (0x2, _) => "Unknown network controller",

            (0x3, 0x0) => "VGA compatible controller",
            (0x3, 0x1) => "XGA controller",
            (0x3, 0x2) => "3D Controller",
            (0x3, 0x80) => "Other display controller",
            (0x3, _) => "Unknown display controller",

            (0x6, 0x0) => "Host bridge",
            (0x6, 0x1) => "ISA bridge",
            (0x6, 0x2) => "EISA bridge",
            (0x6, 0x3) => "MCA bridge",
            (0x6, 0x4) => "PCI-to-PCI bridge",
            (0x6, 0x5) => "PCMCIA bridge",
            (0x6, 0x6) => "NuBus bridge",
            (0x6, 0x7) => "CardBus bridge",
            (0x6, 0x8) => "RACEway bridge",
            (0x6, 0x9) => "PCI-to-PCI bridge",
            (0x6, 0xA) => "InfiniBand-to-PCI host bridge",
            (0x6, 0x80) => "Other bridge",
            (0x6, _) => "Unknown bridge",

            (0xC, 0x0) => "FireWire (IEEE 1394) controller",
            (0xC, 0x1) => "ACCESS bus controller",
            (0xC, 0x2) => "SSA controller",
            (0xC, 0x3) => "USB controller",
            (0xC, 0x4) => "Fibre controller",
            (0xC, 0x5) => "SMBus controller",
            (0xC, 0x6) => "InfiniBand controller",
            (0xC, 0x7) => "IPMI controller",
            (0xC, 0x8) => "SERCOS interface (IEC 61491)",
            (0xC, 0x9) => "CANbus controller",
            (0xC, 0x80) => "Other serial bus controller",
            (0xC, _) => "Unknown serial bus controller",

            (class, subclass) => {
                error!("TODO: PCI class={class:02x} subclass={subclass:02x}");
                "unknown"
            }
        }
    }
}

impl fmt::Display for Device {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let DeviceLocation { bus, device, func } = self.location;
        let subclass_name = self.subclass_name();
        write!(f, "{bus:02x}:{device:02x}.{func} {subclass_name}")
    }
}

impl Deref for Device {
    type Target = DeviceLocation;

    fn deref(&self) -> &Self::Target {
        &self.location
    }
}

impl DerefMut for Device {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.location
    }
}

//

fn pci_config_read_word(bus: u8, slot: u8, func: u8, offs: u8) -> u16 {
    let address = ((bus as u32) << 16)
        | ((slot as u32) << 11)
        | ((func as u32) << 8)
        | ((offs as u32) & 0xFC)
        | 0x80000000u32;

    let mut cfg_addr = Port::<u32>::new(0xCF8);
    let mut cfg_data = Port::<u32>::new(0xCFC);

    unsafe { cfg_addr.write(address) };
    ((unsafe { cfg_data.read() } >> ((offs as u32 & 2) * 8)) & 0xFFFF) as u16
}

fn header_type(bus: u8, slot: u8) -> u8 {
    (pci_config_read_word(bus, slot, 0, 0xA) & 0xFF) as u8
}

//

enum Either<L, R> {
    L(L),
    R(R),
}

impl<L, R> Iterator for Either<L, R>
where
    L: Iterator,
    R: Iterator<Item = L::Item>,
{
    type Item = L::Item;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Either::L(l) => l.next(),
            Either::R(r) => r.next(),
        }
    }
}
