const path = require("path");
const fs = require("fs");
const mitre = require("../../src/ffi/node");

describe("parseConfig", () => {
  it.todo("throws errors on invalid configs");
  it("returns not-null for a not-null config file", () => {
    let p = path.resolve(
      __dirname,
      "../fixtures/example-1-simple-mixed-migrations/mitre.yml"
    );
    fs.accessSync(p, fs.F_OK); // assert we have the right file
    expect(mitre.parseConfig(p)).toBeTruthy()
  });
  
  it("returns a parsed config by dir", () => {
    let p = path.resolve(
      __dirname,
      "../fixtures/example-1-simple-mixed-migrations/mitre.yml"
    );
    let config = mitre.parseConfig(p)
    expect(
      config.migrationsDirectory
    ).toEqual(expect.stringMatching(/.*example-1-simple-mixed-migrations\/migrations\/$/));
    expect(config.configuredRunners).toMatchSnapshot();
  });
});