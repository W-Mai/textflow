#!/usr/bin/env sh
set -eu

cd "$(dirname "$0")"

target_dir="../../target/esp32c3/riscv32imc-unknown-none-elf/release"
cargo build --release --locked --bins

measure_flash() {
    name="$1"
    output="$(espflash save-image --chip esp32c3 --merge --skip-padding \
        "$target_dir/$name" "$target_dir/$name.bin" 2>&1)"
    printf '%s\n' "$output" >&2
    printf '%s\n' "$output" \
        | sed -n 's/.*App\/part\. size:[[:space:]]*\([0-9,]*\)\/.*/\1/p' \
        | tr -d ','
}

section_size() {
    rust-size -A "$target_dir/$1" | awk -v section="$2" '$1 == section { print $2 }'
}

app_flash="$(measure_flash textflow-esp32c3-demo)"
base_flash="$(measure_flash baseline)"
app_data="$(section_size textflow-esp32c3-demo .data)"
app_bss="$(section_size textflow-esp32c3-demo .bss)"
base_data="$(section_size baseline .data)"
base_bss="$(section_size baseline .bss)"
app_static=$((app_data + app_bss))
base_static=$((base_data + base_bss))

printf '\n%-24s %12s %12s\n' "image" "app flash" "static RAM"
printf '%-24s %12s %12s\n' "ESP-HAL baseline" "$base_flash B" "$base_static B"
printf '%-24s %12s %12s\n' "TextFlow pipeline" "$app_flash B" "$app_static B"
printf '%-24s %12s %12s\n' "TextFlow delta" "$((app_flash - base_flash)) B" "$((app_static - base_static)) B"
