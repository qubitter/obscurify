function loadSong() {
  var xhttp = new XMLHttpRequest();
  var pre = '<iframe style="border-radius:12px" src="https://open.spotify.com/embed/track/';
  var post = ' width="80%" height="152" frameBorder="0" allowfullscreen="" allow="autoplay; clipboard-write; encrypted-media; fullscreen; picture-in-picture" loading="lazy"></iframe>';
  xhttp.onreadystatechange = function() {
    if (this.readyState == 4 && this.status == 200) {
      document.getElementById("song").innerHTML = pre + this.responseText + post;
    }
  };
  xhttp.open("GET", "http://140.82.51.99:80/sporify", true);
  xhttp.send();
}
