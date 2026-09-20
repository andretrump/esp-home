function toggle(id) {
    var field = document.getElementById(id);
    if (field) {
        field.type = (field.type === "password") ? "text" : "password";
    }
}
