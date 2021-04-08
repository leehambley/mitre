const mitre = require("../../src/ffi/node");

describe("reserved-words", () => {
  it("exposes reserved words", () => {
    expect(mitre.reservedWords().length).toBe(3);
  });
});
