# ESP32-C3

This `no_std` application runs bidirectional analysis, font selection, shaping, line breaking, visual reordering, glyph positioning, and caret generation with one fixed-capacity `LayoutScratch` value.

```bash
rustup target add riscv32imc-unknown-none-elf
cargo build --release --locked
./measure.sh
```

`measure.sh` builds the TextFlow application and an ESP-HAL-only baseline, generates flashable ESP32-C3 images, and reports application-image and static RAM usage.
