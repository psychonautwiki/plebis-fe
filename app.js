'use strict';

const fs = require('fs');

const elasticsearch = require('elasticsearch');

const express = require('express');
const handlebars = require('handlebars');

const results_tpl = handlebars.compile(fs.readFileSync('./templates/results.hbs').toString());

const esclient = new elasticsearch.Client({
    host: process.env.ES_HOST || 'localhost:9200'
});

const app = express();

app.get('/search', async (req, res) => {
    if (!req.query.q) {
        res.redirect('/');
        return;
    }

    try {
        const results = await esclient.search({
            index: 'reports',
            type: 'report',
            body: {
                query: {
                    multi_match: {
                        query: req.query.q,
                        fields: ['title', 'body^7']
                    }
                },
                highlight: {
                    pre_tags: ["<b>"],
                    post_tags: ["</b>"],
                    fields: {
                        title: {
                            number_of_fragments: 1,
                            fragment_size: 100
                        },
                        body: {
                            fragment_size: 100,
                            number_of_fragments: 3,
                            order: ''
                        }
                    }
                }
            },
            size: 20
        });

        const reports = results.hits.hits.map(result => ({
            title: result.highlight.title ? result.highlight.title.join('') : result._source.title,
            display_text: result.highlight.body ? [...result.highlight.body, ''].join(' … ') : result._source.body.slice(0, 300) + '…',
            link: `http://erowid.org.global.prod.fastly.net/experiences/exp.php?ID=${result._source.meta.erowidId}`,
            tags: [...new Set(result._source.substanceInfo.map(entry => entry.substance))].map(entry => ({ label: entry })),
            obstrusiveTags: [
                result._source.meta.gender,
                result._source.meta.age && `${result._source.meta.age}y`,
                result._source.meta.year
            ].filter(a => a).map(entry => ({label: entry}))
        }));

        res.send(results_tpl({
            reports, query: req.query.q
        }));
    } catch(err) {
        console.error(err);

        res.status(500).send("Something went wrong.");
    }
});

app.use('/', express.static('static/'));

app.listen(8080, () => {
    console.log('Online.');
});