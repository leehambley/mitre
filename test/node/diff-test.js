const path = require("path");
const mitre = require("../../src/ffi/node");

describe("diff", () => {
  it("fails with no config file passed", () => {
    expect(() => mitre.diff()).toThrow();
  });

  it("sends a list of migration states", () => {
    let p = path.resolve(
      __dirname,
      "../fixtures/example-1-simple-mixed-migrations/mitre.yml"
    );
    let config = mitre.parseConfig(p)
    const res = mitre.diff(config);
    expect(res.length).toBe(4);
    for (let i = 0; i < res.length; i++) {
      expect(res[i].state).toMatch(/Applied|Pending/g);
      res[i].migration.steps.forEach(s => {
        expect(Object.keys(s).length).toEqual(1);
        expect(Object.keys(s)[0]).toMatch(/Up|Change|Down/g);
        const direction = Object.keys(s)[0];
        const step = s[direction];
        expect(typeof step.path).toBe('string');
        expect(typeof step.source).toBe('string');
      });
      // local file paths on the steps' paths make it impossible
      // to use snapshoh testing portably
      // expect(res[i].migration.steps).toMatchSnapshot();
    }
  });
});
