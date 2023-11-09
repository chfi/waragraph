import BED from '@gmod/bed';

import '../sidebar-bed.css';

let pathNamesMap = new Map();

function transformBedRange(bed_entry) {
    let name = bed_entry.chrom;

    const regex = /.+:(\d+)-(\d+)$/;

    const found = name.match(regex);

    if (found === null) {
        return bed_entry;
    }

    const start = Number(found[1]);

    const chromStart = bed_entry.chromStart - start;
    const chromEnd = bed_entry.chromEnd - start;

    const new_entry = Object.assign({}, bed_entry, { chromStart, chromEnd });

    return new_entry;
}

async function loadBedFile(file) {
    const bed_list = document.getElementById('bed-file-list');

    const entries_list = document.createElement('div');
    entries_list.classList.add('bed-file-entry');

    const name_el = document.createElement('div');
    name_el.innerHTML = file.name;
    name_el.classList.add('bed-file-name');
    name_el.style.setProperty('flex-basis', '30px');

    entries_list.append(name_el);

    bed_list.append(entries_list);

    const bed_text = await file.text();

    const parser = new BED();

    const bed_lines = bed_text.split('\n').map(line => parser.parseLine(line));

    for (const bed_entry of bed_lines) {

        if (!Number.isNaN(bed_entry.chromStart)
            && !Number.isNaN(bed_entry.chromEnd)) {
            const entry = transformBedRange(bed_entry);

            const entry_div = document.createElement('div');
            entry_div.classList.add('bed-file-row');
            entry_div.innerHTML = entry.name;
            entry_div.addEventListener('click', (e) => {
                console.log("start: " + entry.chromStart + ", end: " + entry.chromEnd);
            });

            entries_list.append(entry_div);
        }

    }

}

function bedSidebarPanel() {
    const bed_pane = document.createElement('div');
    bed_pane.classList.add('bed-panel');

    const bed_list = document.createElement('div');
    bed_list.id = 'bed-file-list';

    bed_pane.append(bed_list);

    const file_label = document.createElement('label');
    file_label.setAttribute('for', 'bed-file-input');
    file_label.innerHTML = 'Load BED file';

    const file_entry = document.createElement('input');
    file_entry.id = 'bed-file-input';
    file_entry.setAttribute('type', 'file');
    file_entry.setAttribute('name', 'bed-file-input');
    file_entry.setAttribute('accept', '.bed');

    const file_button = document.createElement('button');
    file_button.innerHTML = 'Load';

    file_button.addEventListener('click', (ev) => {
        for (const file of file_entry.files) {
            loadBedFile(file);
        }
    });

    bed_pane.append(file_label);
    bed_pane.append(file_entry);
    bed_pane.append(file_button);


    return bed_pane;
}


export async function initializeBedSidebarPanel(worker_obj) {

    let path_names = await worker_obj.getPathNames();
    let path_index = 0;

    for (const name of path_names) {
        pathNamesMap.set(name, path_index);
        path_index += 1;
    }

    document
        .getElementById('sidebar')
        .append(bedSidebarPanel());
}
