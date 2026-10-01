// Conveniences only. Every page works without this file.
var said = document.querySelector('[role="status"]');
function say(words) {
  // A live region: a screen reader says what is written into it.
  if (said) said.textContent = words;
}
document.querySelectorAll('[data-copy-text]').forEach(function (button) {
  if (!navigator.clipboard) return;
  button.hidden = false;
  button.addEventListener('click', function () {
    navigator.clipboard.writeText(button.getAttribute('data-copy-text')).then(
      function () {
        say('Copied');
      },
      function () {
        say('Copying did not work. Select the text and copy it.');
      }
    );
  });
});
document.querySelectorAll('[data-print]').forEach(function (button) {
  button.hidden = false;
  button.addEventListener('click', function () {
    window.print();
  });
});
