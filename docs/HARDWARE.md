# Hardware Reference

Technical reference for Raspberry Pi 5 hardware as used by Intent Kernel.

## Raspberry Pi 5 Specifications

| Component | Specification |
|-----------|--------------|
| SoC | Broadcom BCM2712 |
| CPU | Quad-core ARM Cortex-A76 @ 2.4GHz |
| GPU | VideoCore VII |
| RAM | 4GB or 8GB LPDDR4X-4267 |
| Storage | MicroSD, NVMe (via PCIe) |
| USB | 2× USB 3.0, 2× USB 2.0 |
| Ethernet | Gigabit with PoE+ support |
| Display | 2× micro-HDMI 4Kp60 |
| GPIO | 40-pin header |

## BCM2712 Memory Map

### Peripheral Base Addresses

| Region | Address | Size | Description |
|--------|---------|------|-------------|
| Peripheral Base | `0x1_0000_0000` | 128MB | Main peripherals |
| GPIO | `0x1_0020_0000` | 4KB | GPIO controller |
| UART0 (PL011) | `0x1_0020_1000` | 4KB | Primary UART |
| SPI0 | `0x1_0020_4000` | 4KB | SPI master 0 |
| I2C0 | `0x1_0020_5000` | 4KB | I2C master 0 |
| AUX | `0x1_0021_5000` | 4KB | Mini UART, SPI1/2 |
| System Timer | `0x1_0000_3000` | 4KB | BCM timer |
| IRQ Controller | `0x1_0000_B200` | 4KB | Legacy IRQ |
| Mailbox | `0x1_0000_B880` | 4KB | VideoCore mailbox |
| GIC-400 | `0x1_0004_0000` | 64KB | ARM GIC |
| GICD | `0x1_0004_1000` | 4KB | GIC Distributor |
| GICC | `0x1_0004_2000` | 4KB | GIC CPU Interface |

### ARM Memory Map

```
0x0000_0000_0000 ┌─────────────────────────┐
                 │ Reserved (VideoCore)    │ 512KB
0x0000_0008_0000 ├─────────────────────────┤
                 │ Kernel Load Point       │ ← _start entry
                 │ .text.boot              │
                 │ .text                   │
                 │ .rodata                 │
                 │ .data                   │
                 │ .bss                    │
0x0000_0020_0000 ├─────────────────────────┤
                 │ Heap / Capability Pool  │
                 │                         │
                 │ (grows upward)          │
                 │                         │
0x0001_0000_0000 ├─────────────────────────┤
                 │ Peripherals             │ 128MB
0x0001_0800_0000 ├─────────────────────────┤
                 │ PCIe, etc.              │
                 └─────────────────────────┘
```

## GPIO

### GPIO Pin Header (40-pin)

```
                    3V3  (1) (2)  5V
          GPIO2/SDA1  (3) (4)  5V
         GPIO3/SCL1  (5) (6)  GND
             GPIO4  (7) (8)  GPIO14/TXD0
                GND  (9) (10) GPIO15/RXD0
            GPIO17 (11) (12) GPIO18/PCM_CLK
            GPIO27 (13) (14) GND
            GPIO22 (15) (16) GPIO23
               3V3 (17) (18) GPIO24
   GPIO10/SPI_MOSI (19) (20) GND
   GPIO9/SPI_MISO  (21) (22) GPIO25
   GPIO11/SPI_SCLK (23) (24) GPIO8/SPI_CE0
               GND (25) (26) GPIO7/SPI_CE1
          GPIO0/ID_SD (27) (28) GPIO1/ID_SC
             GPIO5 (29) (30) GND
             GPIO6 (31) (32) GPIO12/PWM0
       GPIO13/PWM1 (33) (34) GND
   GPIO19/PCM_FS   (35) (36) GPIO16
            GPIO26 (37) (38) GPIO20/PCM_DIN
               GND (39) (40) GPIO21/PCM_DOUT
```

### GPIO Function Select

Each GPIO pin can be configured for one of 8 functions:

| Value | Function |
|-------|----------|
| 0b000 | Input |
| 0b001 | Output |
| 0b010 | Alt5 |
| 0b011 | Alt4 |
| 0b100 | Alt0 |
| 0b101 | Alt1 |
| 0b110 | Alt2 |
| 0b111 | Alt3 |

### GPIO Registers (BCM2712)

| Offset | Name | Description |
|--------|------|-------------|
| 0x00 | GPFSEL0 | Function select for GPIO 0-9 |
| 0x04 | GPFSEL1 | Function select for GPIO 10-19 |
| 0x08 | GPFSEL2 | Function select for GPIO 20-29 |
| 0x0C | GPFSEL3 | Function select for GPIO 30-39 |
| 0x10 | GPFSEL4 | Function select for GPIO 40-49 |
| 0x14 | GPFSEL5 | Function select for GPIO 50-57 |
| 0x1C | GPSET0 | Output set for GPIO 0-31 |
| 0x20 | GPSET1 | Output set for GPIO 32-57 |
| 0x28 | GPCLR0 | Output clear for GPIO 0-31 |
| 0x2C | GPCLR1 | Output clear for GPIO 32-57 |
| 0x34 | GPLEV0 | Pin level for GPIO 0-31 |
| 0x38 | GPLEV1 | Pin level for GPIO 32-57 |
| 0xE4 | PUP_PDN0 | Pull-up/down for GPIO 0-15 |
| 0xE8 | PUP_PDN1 | Pull-up/down for GPIO 16-31 |
| 0xEC | PUP_PDN2 | Pull-up/down for GPIO 32-47 |
| 0xF0 | PUP_PDN3 | Pull-up/down for GPIO 48-57 |

### Special GPIO Pins

| GPIO | Function |
|------|----------|
| 14 | UART0 TXD |
| 15 | UART0 RXD |
| 42 | Activity LED |
| 43 | Power LED |

## UART (PL011)

### PL011 Registers

| Offset | Name | Description |
|--------|------|-------------|
| 0x00 | DR | Data Register |
| 0x04 | RSRECR | Receive Status/Error Clear |
| 0x18 | FR | Flag Register |
| 0x24 | IBRD | Integer Baud Rate Divisor |
| 0x28 | FBRD | Fractional Baud Rate Divisor |
| 0x2C | LCR_H | Line Control Register |
| 0x30 | CR | Control Register |
| 0x38 | IMSC | Interrupt Mask Set/Clear |
| 0x40 | MIS | Masked Interrupt Status |
| 0x44 | ICR | Interrupt Clear Register |

### Flag Register (FR) Bits

| Bit | Name | Description |
|-----|------|-------------|
| 7 | TXFE | Transmit FIFO empty |
| 6 | RXFF | Receive FIFO full |
| 5 | TXFF | Transmit FIFO full |
| 4 | RXFE | Receive FIFO empty |
| 3 | BUSY | UART busy |

### Baud Rate Calculation

```
UART Clock = 48MHz (typical for Pi 5)
Divisor = UART_CLK / (16 × Baud)

For 115200 baud:
Divisor = 48000000 / (16 × 115200) = 26.0417
IBRD = 26
FBRD = (0.0417 × 64) = 2.67 ≈ 3
```

## GIC-400 (ARM Generic Interrupt Controller)

### Interrupt Types

| Range | Type | Description |
|-------|------|-------------|
| 0-15 | SGI | Software Generated Interrupts |
| 16-31 | PPI | Private Peripheral Interrupts |
| 32+ | SPI | Shared Peripheral Interrupts |

### Key Interrupt Numbers

| IRQ | Source |
|-----|--------|
| 27 | Virtual Timer (PPI) |
| 30 | Physical Timer (PPI) |
| 96 | System Timer |
| 97 | Mailbox |
| 113-115 | GPIO Banks |
| 153 | UART0 |

### GICD Registers

| Offset | Name | Description |
|--------|------|-------------|
| 0x000 | CTLR | Distributor Control |
| 0x004 | TYPER | Interrupt Controller Type |
| 0x100 | ISENABLERn | Interrupt Set-Enable |
| 0x180 | ICENABLERn | Interrupt Clear-Enable |
| 0x200 | ISPENDRn | Interrupt Set-Pending |
| 0x280 | ICPENDRn | Interrupt Clear-Pending |
| 0x400 | IPRIORITYRn | Interrupt Priority |
| 0x800 | ITARGETSRn | Interrupt Processor Targets |
| 0xC00 | ICFGRn | Interrupt Configuration |

### GICC Registers

| Offset | Name | Description |
|--------|------|-------------|
| 0x000 | CTLR | CPU Interface Control |
| 0x004 | PMR | Interrupt Priority Mask |
| 0x00C | IAR | Interrupt Acknowledge |
| 0x010 | EOIR | End of Interrupt |
| 0x014 | RPR | Running Priority |

## VideoCore Mailbox

### Mailbox Registers

| Offset | Name | Description |
|--------|------|-------------|
| 0x00 | READ | Receive from VC |
| 0x10 | POLL | Receive without removing |
| 0x14 | SENDER | Sender info |
| 0x18 | STATUS | Mailbox status |
| 0x1C | CONFIG | Mailbox config |
| 0x20 | WRITE | Send to VC |

### Status Bits

| Bit | Name | Description |
|-----|------|-------------|
| 31 | FULL | Cannot write |
| 30 | EMPTY | Cannot read |

### Mailbox Channels

| Channel | Purpose |
|---------|---------|
| 0 | Power management |
| 1 | Framebuffer |
| 2 | Virtual UART |
| 3 | VCHIQ |
| 4 | LEDs |
| 5 | Buttons |
| 6 | Touch screen |
| 8 | Property tags (ARM→VC) |
| 9 | Property tags (VC→ARM) |

### Property Tags

| Tag | Description |
|-----|-------------|
| 0x00000001 | Get firmware revision |
| 0x00010001 | Get board model |
| 0x00010002 | Get board revision |
| 0x00010005 | Get ARM memory |
| 0x00010006 | Get VC memory |
| 0x00030006 | Get temperature |
| 0x00030002 | Get clock rate |
| 0x00038002 | Set clock rate |
| 0x00040001 | Allocate framebuffer |
| 0x00048003 | Set physical display size |
| 0x00048004 | Set virtual display size |
| 0x00048005 | Set depth |
| 0x00040008 | Get pitch |

## ARM Timer

### ARM Generic Timer

Intent Kernel uses the ARM Generic Timer (architected timer) rather than the BCM system timer.

Access via system registers:
- `CNTFRQ_EL0` - Counter frequency
- `CNTPCT_EL0` - Physical counter value
- `CNTP_TVAL_EL0` - Physical timer value
- `CNTP_CTL_EL0` - Physical timer control
- `CNTP_CVAL_EL0` - Physical timer compare value

### Timer Frequency

Pi 5 typically runs at 54MHz counter frequency.

```
Ticks per microsecond = 54
Ticks per millisecond = 54,000
Ticks per second = 54,000,000
```

---

## References

- [BCM2712 Datasheet](https://datasheets.raspberrypi.com/bcm2712/bcm2712-peripherals.pdf) (when available)
- [ARM Cortex-A76 Technical Reference Manual](https://developer.arm.com/documentation/100798/latest)
- [ARM GIC-400 Technical Reference Manual](https://developer.arm.com/documentation/ddi0471/latest)
- [Raspberry Pi Documentation](https://www.raspberrypi.com/documentation/)

---

## USB Host Controller (xHCI)

The Raspberry Pi 5 uses an xHCI-compliant USB 3.0 controller via the RP1 southbridge.

### USB Controller Addresses

| Region | Address | Description |
|--------|---------|-------------|
| xHCI Base | `0x1_00200000` | xHCI Controller (via RP1 PCIe) |
| Capability | +0x00 | Capability registers |
| Operational | +0x80 | Operational registers |
| Runtime | +0x600 | Runtime registers |
| Doorbell | +0x800 | Doorbell registers |

### xHCI Registers

| Offset | Name | Description |
|--------|------|-------------|
| 0x00 | CAPLENGTH | Capability Register Length |
| 0x04 | HCSPARAMS1 | Structural Parameters 1 |
| 0x08 | HCSPARAMS2 | Structural Parameters 2 |
| 0x10 | HCCPARAMS1 | Capability Parameters 1 |
| 0x14 | DBOFF | Doorbell Offset |
| 0x18 | RTSOFF | Runtime Register Space Offset |

### Operational Registers

| Offset | Name | Description |
|--------|------|-------------|
| 0x00 | USBCMD | USB Command |
| 0x04 | USBSTS | USB Status |
| 0x08 | PAGESIZE | Page Size |
| 0x14 | DNCTRL | Device Notification Control |
| 0x18 | CRCR | Command Ring Control |
| 0x30 | DCBAAP | Device Context Base Address Array Pointer |
| 0x38 | CONFIG | Configure |

### Port Registers (per port)

| Offset | Name | Description |
|--------|------|-------------|
| 0x00 | PORTSC | Port Status and Control |
| 0x04 | PORTPMSC | Port Power Management |
| 0x08 | PORTLI | Port Link Info |

### HID Protocol for Steno Machines

Steno machines using Plover HID protocol send 6-byte reports:

```
Byte 0: Report ID (0x50 for steno)
Byte 1-4: 32-bit stroke data (little-endian)
Byte 5: Flags
```

Stroke data layout (Plover HID):
```
Bit 0-22: Stroke keys (23-bit pattern)
Bit 23-31: Reserved
```

### Supported Devices

| Device | VID | PID | Protocol |
|--------|-----|-----|----------|
| Georgi | 0x1209 | 0x2303 | Plover HID |
| Uni | 0x1209 | 0x2301 | Plover HID |
| SOFT/HRUF | 0x4D3 | 0xD1 | NKRO |

---

*Last updated: December 2025*
