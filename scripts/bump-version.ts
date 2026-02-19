import { join } from "path";
import { readFileSync, writeFileSync } from "fs";
import { spawnSync } from "child_process";

const args = process.argv.slice(2);
const type = args[0] as "patch" | "minor" | "major";

if (!["patch", "minor", "major"].includes(type)) {
  console.error("Usage: bun scripts/bump-version.ts <patch|minor|major>");
  process.exit(1);
}

const root = process.cwd();
const packageJsonPath = join(root, "package.json");
const tauriConfPath = join(root, "src-tauri", "tauri.conf.json");
const cargoTomlPath = join(root, "src-tauri", "Cargo.toml");

// 1. Read current version from package.json
const packageJson = JSON.parse(readFileSync(packageJsonPath, "utf-8"));
const currentVersion = packageJson.version;
console.log(`Current version: ${currentVersion}`);

const [major, minor, patch] = currentVersion.split(".").map(Number);

let newVersion = "";
if (type === "patch") newVersion = `${major}.${minor}.${patch + 1}`;
if (type === "minor") newVersion = `${major}.${minor + 1}.0`;
if (type === "major") newVersion = `${major + 1}.0.0`;

console.log(`New version: ${newVersion}`);

// 2. Update package.json
packageJson.version = newVersion;
writeFileSync(packageJsonPath, JSON.stringify(packageJson, null, 2) + "\n");
console.log(`Updated ${packageJsonPath}`);

// 3. Update tauri.conf.json
const tauriConf = JSON.parse(readFileSync(tauriConfPath, "utf-8"));
tauriConf.version = newVersion;
writeFileSync(tauriConfPath, JSON.stringify(tauriConf, null, 2) + "\n");
console.log(`Updated ${tauriConfPath}`);

// 4. Update Cargo.toml
let cargoToml = readFileSync(cargoTomlPath, "utf-8");
// Replace version = "x.y.z" under [package]
// We use a regex dealing with the specific format in Cargo.toml
const cargoRegex = /^version\s*=\s*"[^"]+"/m;
if (cargoRegex.test(cargoToml)) {
    cargoToml = cargoToml.replace(cargoRegex, `version = "${newVersion}"`);
    writeFileSync(cargoTomlPath, cargoToml);
    console.log(`Updated ${cargoTomlPath}`);
} else {
    console.error("Could not find version in Cargo.toml");
    process.exit(1);
}

// 5. Git operations
function run(cmd: string, args: string[]) {
    console.log(`Running: ${cmd} ${args.join(" ")}`);
    const result = spawnSync(cmd, args, { stdio: "inherit" });
    if (result.status !== 0) {
        console.error("Command failed");
        process.exit(1);
    }
}

run("git", ["add", packageJsonPath, tauriConfPath, cargoTomlPath]);
run("git", ["commit", "-m", `v${newVersion}`]);
run("git", ["tag", `v${newVersion}`]);

console.log(`Successfully bumped to v${newVersion}`);
