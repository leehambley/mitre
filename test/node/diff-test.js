const path = require("path");
const mitre = require("../../src/ffi/node");

describe("diff", () => {
  it.skip("fails with no config file passed", () => {
    // expect(() => mitre.diff()).toThrow();
  });

  it("sends a list of migration states", () => {
    // let p = path.resolve(
    //   __dirname,
    //   "../fixtures/example-1-simple-mixed-migrations/mitre.yml"
    // );
    // let config = mitre.parseConfig(p)
    // const res = mitre.diff(config);
    // expect(res.length).toBe(3);
    // expect(res[0].state).toBe("Pending")
  });
});
