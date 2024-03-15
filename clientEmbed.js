function loadSong() {
  var xhttp = new XMLHttpRequest();
  var pre = '<iframe style="border-radius:12px" src="https://open.spotify.com/embed/track/';
  var post = ' width="80%" height="152" frameBorder="0" allowfullscreen="" allow="autoplay; clipboard-write; encrypted-media; fullscreen; picture-in-picture" loading="lazy"></iframe>';
  xhttp.onreadystatechange = function() {
    if (this.readyState == 4) {
      switch (this.status) {
        case 200:
          document.getElementById("song").innerHTML = pre + this.responseText + post;
          break;
        case 204:
          document.getElementById("song").innerHTML = "Nothing right now! Check back later."
          break;
        case 503:
          document.getElementById("song").innerHTML = "Obscurify is down! Check back later."
          break;
      }
    }
  };
  xhttp.open("GET", "https://127.0.0.1/spotify", true);
  // xhttp.setRequestHeader("Access-Control-Allow-Origin", "*");
  xhttp.send();
}
