{{ include_raw("dist/lecturenotes.js") }}
window.flmLecturenotes.setup({{ {"navigation_heading_level": config.navigation_heading_level} | tojson }});
