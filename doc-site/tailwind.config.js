// no perfect matchng color names, e.g 
// https://shallowsky.com/colormatch/index.php?hex=6d77b0
// https://shallowsky.com/colormatch/index.php?hex=4ab7c3
module.exports = {
  purge: [
    './src/**/*.html',
    './src/**/*.js',
  ],
  darkMode: true,
  theme: {
    colors: {
      slatev: {
        dark: '#6d77b0',
        light: '#aeb6d8',
      },
      turquoise: {
        dark: '#4ab7c3',
        light: '#9ad8e1'
      },
      textGray: {
        DEFAULT: '#'
      }
    }
  },
}