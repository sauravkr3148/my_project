export default class topBar {
  constructor(name) {
    this.Name = name;
    this.updateName();
    this.initEventHandlers();
    document.getElementById("menucir").classList.add('hidden');
  }

  updateName() {
    document.getElementById("nctitle").innerText = this.Name;
  }

  initEventHandlers() {
    let hideTimeout;

    document.querySelector(".ncclose").addEventListener("click", function () {
      alert("Disconnecting current session");
      window.close();
    });

    const hideToolbar = () => {
      document.getElementById("toolbar").classList.add('hidden');
      document.getElementById("menucir").classList.remove('hidden');
    };

    const showToolbar = () => {
      document.getElementById("menucir").classList.add('hidden');
      document.getElementById("toolbar").classList.remove('hidden');
      clearTimeout(hideTimeout);
      hideTimeout = setTimeout(hideToolbar, 5000);
    };

    document.querySelector(".ncpin").addEventListener("click", function () {
      hideToolbar();
    });
    document.querySelector(".ncfullscreen").addEventListener("click", function () {
      const deskParent = document.getElementById("DeskParent");
      if (!document.fullscreenElement) {
        deskParent.requestFullscreen();
      } else {
        document.exitFullscreen();
      }
    });



    document.querySelector(".ncmenu").addEventListener("click", function () {
      let tool = document.getElementById("tool");
      tool.style.display = (tool.style.display === "none" || tool.style.display === "") ? "block" : "none";
    });

    document.querySelector(".menucir").addEventListener("click", function () {
      showToolbar();
    });

    // Keyboard lock & unlock on fullscreen change + show toolbar on exit
    const isKeyboardLockSupported = navigator.userAgent.indexOf("Chrome/") > -1 &&
      navigator.userAgent.indexOf("Edge/") === -1;

    if (isKeyboardLockSupported && navigator.keyboard?.lock) {
      document.addEventListener("fullscreenchange", () => {
        const isFullscreen = !!document.fullscreenElement;

        const elementsToHide = [
          "toolbar",
          "topbar-nav",
          "menucir",
          "tool",
          "quality-control",
          "filetransfer-view",
          "join-menu"
        ];

        elementsToHide.forEach(id => {
          const el = document.getElementById(id);
          if (el) {
            if (isFullscreen) {
              el.style.display = "none";
            } else {
              // Special handling: keep join-menu hidden after fullscreen
              if (id === "join-menu") {
                el.style.display = "none";
              } else if (id === "topbar-nav" || id === "toolbar") {
                el.style.display = "flex";
              } else {
                el.style.display = "";
              }
            }
          }
        });

        // Keyboard lock toggle
        if (navigator.keyboard?.lock && isFullscreen) {
          navigator.keyboard.lock().catch(err => {
            console.warn("Keyboard lock failed:", err);
          });
        } else if (navigator.keyboard?.unlock && !isFullscreen) {
          navigator.keyboard.unlock();
        }
      });



    }
  }
}
