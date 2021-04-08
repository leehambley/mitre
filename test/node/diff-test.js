const path = require("path");
const mitre = require("../../src/ffi/node");

describe.skip("diff", () => {
  it("fails with no config file passed", () => {
    expect(() => mitre.diff()).toThrow();
  });

  it("sends a list of migration states", () => {
    const res = mitre.diff({
      migrationsDirectory: path.resolve(
        __dirname,
        "../fixtures/example-1-simple-mixed-migrations/migrations"
      ),
      configuredRunners: {
        mitre: {
          _runner: "mariadb",
          database: "mitre",
          ipOrHostname: "127.0.0.1",
          password: "example",
          port: "3306",
          username: "root",
        },
      },
    });
    expect(res.length).toBe(3);
  });
});
