const path = require("path");
const mitre = require("../../src/ffi/node");

describe("parseConfig", () => {
  it.todo("throws errors on invalid configs");
  it("returns a parsed config by dir", () => {
    expect(
      mitre.parseConfig(
        path.resolve(
          __dirname,
          "../fixtures/example-1-simple-mixed-migrations/mitre.yml"
        )
      )
    ).toMatchInlineSnapshot(`
      Object {
        "configured_runners": Array [],
        "migrations_directory": null,
        "number_of_configured_runners": 0,
      }
    `);
  });
});
