// Build controls panel
// TODO: extract to method reusable statements
export async function controlSidebarPanel(waragraph) {

    // Method to create a label
    function createLabel(text, htmlFor, height = '50%') {
        const label = document.createElement('label');
        label.textContent = text;
        label.htmlFor = htmlFor;
        label.classList.add('full-width');
        label.style.height = height;
        return label;
    }

    // Method to create an input
    function createInput(id, placeholder) {
        const input = document.createElement('input');
        input.type = 'text';
        input.id = id;
        input.placeholder = placeholder;
        input.setAttribute('inputmode', 'numeric');
        input.setAttribute('pattern', '\\d*');
        input.setAttribute('min', '0');
        input.setAttribute('step', '1');
        input.classList.add('full-width');
        return input;
    }

    // Method to create a button
    function createButton(id, text) {
        const button = document.createElement('button');
        button.type = 'button';
        button.id = id;
        button.classList.add('full-width');
        button.textContent = text;
        return button;
    }

    function createSpacer() {
        const spacer = document.createElement('div');
        spacer.classList.add('col-1');
        return spacer;
    }

    const controls_div = document.createElement('div');

    // 1D control creation
    controls_div.classList.add('bed-panel');

    const pane_title = document.createElement('h5');
    pane_title.innerHTML = 'Graph Controls';
    pane_title.classList.add('mt-2');

    const break_el = document.createElement('hr');
    break_el.classList.add('my-1');

    // label for hiding/showing 1d controls
    const i_label = document.createElement('label');
    i_label.innerHTML = '►  1D graph controls';
    i_label.classList.add('strong');
    i_label.classList.add('pointer');
    i_label.classList.add('control-row-label');


    // label for hiding/showing 2d controls
    const ii_label = document.createElement('label');
    ii_label.innerHTML = '►  2D graph controls';
    ii_label.classList.add('strong');
    ii_label.classList.add('pointer');
    ii_label.classList.add('control-row-label');

    // container for 1d controls
    const i_controls = document.createElement('div');
    i_controls.style.display = 'none';
    i_controls.classList.add('control-dropdown');

    // container for 2d controls
    const ii_controls = document.createElement('div');
    ii_controls.style.display = 'none';

    const control_range_label = document.createElement('label');
    control_range_label.innerHTML = 'Jump to 1D range:';
    control_range_label.classList.add('mb-1');

    const range_input_row = document.createElement('div');
    range_input_row.classList.add('row');

    const label_div = document.createElement('div');
    label_div.title = 'label-group';
    label_div.classList.add('col-2');

    const input_div = document.createElement('div');
    input_div.title = 'input-group';
    input_div.classList.add('col-8');

    const input_group = document.createElement('div');
    input_group.title = 'input-group';
    input_group.classList.add('col-12');

    const label_start = createLabel('Start:', 'control-input-range-start');
    const input_start = createInput('control-input-range-start', 'Start');
    const label_end = createLabel('End:', 'control-input-range-end');
    const input_end = createInput('control-input-range-end', 'End');
    const input_button = createButton('control-input-range-button', 'Go');

    // Segment control creation
    const control_segment_label = document.createElement('label');
    control_segment_label.innerHTML = 'Jump to 2D segment:';

    const segment_input_row = document.createElement('div');
    segment_input_row.classList.add('row');

    const label_div_segment_start = document.createElement('div');
    label_div_segment_start.title = 'label-group-segment';
    label_div_segment_start.classList.add('col-4');

    const input_div_segment = document.createElement('div');
    input_div_segment.title = 'input-group-segment';
    input_div_segment.classList.add('col-6');

    const input_group_segment = document.createElement('div');
    input_group_segment.classList.add('col-12');

    const label_segment = createLabel('Segment:', 'control-input-segment-start');
    const input_start_segment = createInput('control-input-segment-start', '0');
    const input_button_segment = createButton('control-input-segment-button', 'Go');



    // Populate child divs
    label_div.appendChild(label_start);
    label_div.appendChild(label_end);

    input_div.appendChild(input_start);
    input_div.appendChild(input_end);

    var spacer = createSpacer();
    range_input_row.appendChild(spacer);
    range_input_row.appendChild(label_div);
    range_input_row.appendChild(input_div);
    input_group.appendChild(input_button);
    input_group.classList.add('m-1');


    spacer = createSpacer();
    segment_input_row.appendChild(spacer);
    segment_input_row.appendChild(label_div_segment_start);
    segment_input_row.appendChild(input_div_segment);

    i_controls.appendChild(control_range_label);
    i_controls.appendChild(range_input_row);
    i_controls.appendChild(input_group);

    label_div_segment_start.appendChild(label_segment);
    input_div_segment.appendChild(input_start_segment);



    input_group_segment.appendChild(input_button_segment);
    input_group_segment.classList.add('m-1');

    ii_controls.appendChild(control_segment_label);
    ii_controls.appendChild(segment_input_row);
    ii_controls.appendChild(input_group_segment);


    // Populate parent div
    controls_div.appendChild(pane_title);
    controls_div.appendChild(break_el);
    controls_div.appendChild(i_label);
    controls_div.appendChild(i_controls);
    controls_div.appendChild(ii_label);
    controls_div.appendChild(ii_controls);


    i_label.addEventListener('click', (ev) => {
        if (i_controls.style.display === 'block') {
            i_controls.style.display = 'none';
            i_label.innerHTML = '►  1D graph controls';
        }
        else {
            i_controls.style.display = 'block';
            i_label.innerHTML = '▼  1D graph controls';
            i_controls.style.marginLeft = '20px';
        }
    });

    ii_label.addEventListener('click', (ev) => {
        if (ii_controls.style.display === 'block') {
            ii_controls.style.display = 'none';
            ii_label.innerHTML = '►  2D graph controls';
        }
        else {
            ii_controls.style.display = 'block';
            ii_label.innerHTML = '▼  2D graph controls';
            ii_controls.style.marginLeft = '20px';
        }
    });

    return controls_div;
}
