{{ include_raw("dist/lecturenotes.js") }}
window.flmLecturenotes.setup({{ config | tojson }});
